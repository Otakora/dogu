from __future__ import annotations

import json
import os
import platform
import re
import shutil
import subprocess
import zipfile
from dataclasses import dataclass, field
from datetime import datetime
from pathlib import Path
from typing import Callable, Iterable, Literal

from .paths import get_platform_name


ProgressCallback = Callable[[float, str], None]
LogCallback = Callable[[str], None]

ARCHIVE_EXTENSIONS = {".zip", ".7z", ".rar"}
CD_SOURCE_EXTENSIONS = {".cue", ".gdi", ".toc"}
DVD_SOURCE_EXTENSIONS = {".iso"}
PAIRABLE_BIN_EXTENSIONS = {".bin"}

MAME_RELEASE_URL = "https://www.mamedev.org/release.html"
MAME_CHDMAN_DOCS_URL = "https://docs.mamedev.org/tools/chdman.html"
MAME_GITHUB_URL = "https://github.com/mamedev/mame"
CHDMAN_METADATA_PATH = Path("third_party/chdman/metadata.json")


@dataclass(slots=True)
class FileEntry:
    path: Path
    is_dir: bool
    size: int
    modified_ts: float

    @property
    def name(self) -> str:
        return self.path.name or str(self.path)

    @property
    def suffix(self) -> str:
        return self.path.suffix.lower()

    @property
    def modified_label(self) -> str:
        return format_timestamp(self.modified_ts)

    @property
    def size_label(self) -> str:
        return format_size(self.size) if not self.is_dir else ""


@dataclass(slots=True)
class ClipboardPayload:
    paths: list[Path]
    operation: Literal["copy", "cut"]


@dataclass(slots=True)
class ExtractionOptions:
    individual_folders: bool = False
    destination_mode: Literal["same", "custom"] = "same"
    destination_path: Path | None = None
    delete_archives: bool = False
    overwrite: bool = False


@dataclass(slots=True)
class ChdConversionSource:
    command: Literal["createcd", "createdvd"]
    source_path: Path
    required_paths: list[Path]
    display_extensions: tuple[str, ...]
    container_dir: Path


@dataclass(slots=True)
class ChdConversionOptions:
    delete_originals: bool = False
    name_as_container: bool = False
    deposit_to_parent: bool = False
    delete_original_subfolders: bool = False
    overwrite: bool = False


@dataclass(slots=True)
class PropertiesSummary:
    count: int
    files: int
    directories: int
    total_size: int
    lines: list[str] = field(default_factory=list)


def format_size(size: int) -> str:
    units = ["B", "KB", "MB", "GB", "TB"]
    value = float(size)
    for unit in units:
        if value < 1024 or unit == units[-1]:
            if unit == "B":
                return f"{int(value)} {unit}"
            return f"{value:.2f} {unit}"
        value /= 1024
    return f"{size} B"


def format_timestamp(timestamp: float) -> str:
    try:
        return datetime.fromtimestamp(timestamp).strftime("%Y-%m-%d %H:%M:%S")
    except (OSError, ValueError):
        return "-"


def safe_stat(path: Path) -> os.stat_result | None:
    try:
        return path.stat()
    except OSError:
        return None


def build_entry(path: Path) -> FileEntry:
    stat = safe_stat(path)
    return FileEntry(
        path=path,
        is_dir=path.is_dir(),
        size=0 if stat is None or path.is_dir() else stat.st_size,
        modified_ts=0 if stat is None else stat.st_mtime,
    )


def list_directory(path: Path) -> list[FileEntry]:
    entries: list[FileEntry] = []
    try:
        with os.scandir(path) as scan:
            for item in scan:
                item_path = Path(item.path)
                try:
                    stat = item.stat()
                except OSError:
                    continue
                entries.append(
                    FileEntry(
                        path=item_path,
                        is_dir=item.is_dir(),
                        size=0 if item.is_dir() else stat.st_size,
                        modified_ts=stat.st_mtime,
                    )
                )
    except OSError as exc:
        raise RuntimeError(f"No se pudo leer la carpeta: {path}\n{exc}") from exc

    entries.sort(key=lambda item: (not item.is_dir, item.name.lower()))
    return entries


def iter_search(root: Path, query: str, recursive: bool = True) -> list[FileEntry]:
    lowered = query.casefold()
    results: list[FileEntry] = []

    def matches(path: Path) -> bool:
        return lowered in path.name.casefold()

    if recursive:
        for current_root, dirnames, filenames in os.walk(root):
            base = Path(current_root)
            for dirname in dirnames:
                child = base / dirname
                if matches(child):
                    results.append(build_entry(child))
            for filename in filenames:
                child = base / filename
                if matches(child):
                    results.append(build_entry(child))
    else:
        for entry in list_directory(root):
            if matches(entry.path):
                results.append(entry)

    results.sort(key=lambda item: (not item.is_dir, item.name.lower()))
    return results


def summarize_paths(paths: Iterable[Path]) -> PropertiesSummary:
    lines: list[str] = []
    total_size = 0
    file_count = 0
    directory_count = 0
    normalized = [Path(path) for path in paths]

    for path in normalized:
        if path.is_dir():
            directory_count += 1
            folder_size, folder_files, folder_dirs = compute_directory_stats(path)
            total_size += folder_size
            lines.append(
                f"[DIR] {path.name} | {path} | {format_size(folder_size)} | "
                f"{folder_files} ficheros, {folder_dirs} subcarpetas"
            )
        else:
            file_count += 1
            size = safe_stat(path).st_size if safe_stat(path) else 0
            total_size += size
            lines.append(f"[FILE] {path.name} | {path} | {format_size(size)}")

    return PropertiesSummary(
        count=len(normalized),
        files=file_count,
        directories=directory_count,
        total_size=total_size,
        lines=lines,
    )


def compute_directory_stats(path: Path) -> tuple[int, int, int]:
    total_size = 0
    file_count = 0
    directory_count = 0
    for root, dirnames, filenames in os.walk(path):
        directory_count += len(dirnames)
        for filename in filenames:
            file_count += 1
            file_path = Path(root) / filename
            stat = safe_stat(file_path)
            if stat is not None:
                total_size += stat.st_size
    return total_size, file_count, directory_count


def ensure_default_extension(name: str) -> str:
    return name if Path(name).suffix else f"{name}.txt"


def create_folder(parent: Path, name: str) -> Path:
    target = parent / name
    target.mkdir(parents=False, exist_ok=False)
    return target


def create_file(parent: Path, name: str) -> Path:
    target = parent / ensure_default_extension(name)
    target.write_text("", encoding="utf-8")
    return target


def rename_path(path: Path, new_name: str) -> Path:
    target = path.with_name(new_name)
    path.rename(target)
    return target


def remove_paths(paths: Iterable[Path], progress: ProgressCallback, log: LogCallback) -> None:
    items = list(paths)
    total = max(len(items), 1)
    for index, path in enumerate(items, start=1):
        log(f"Eliminando {path}")
        if path.is_dir():
            shutil.rmtree(path)
        else:
            path.unlink(missing_ok=False)
        progress(index / total, f"Eliminando {path.name}")


def copy_or_move_paths(
    paths: Iterable[Path],
    destination_dir: Path,
    operation: Literal["copy", "cut"],
    overwrite: bool,
    progress: ProgressCallback,
    log: LogCallback,
) -> None:
    items = list(paths)
    total = max(len(items), 1)
    for index, source in enumerate(items, start=1):
        target = destination_dir / source.name
        if target.exists():
            if not overwrite:
                raise FileExistsError(f"El destino ya existe: {target}")
            if target.is_dir():
                shutil.rmtree(target)
            else:
                target.unlink()

        log(f"{'Moviendo' if operation == 'cut' else 'Copiando'} {source} -> {target}")
        if operation == "cut":
            shutil.move(str(source), str(target))
        else:
            if source.is_dir():
                shutil.copytree(source, target)
            else:
                shutil.copy2(source, target)
        progress(index / total, f"{'Moviendo' if operation == 'cut' else 'Copiando'} {source.name}")


def open_with_system(path: Path) -> None:
    system_name = platform.system()
    if system_name == "Windows":
        os.startfile(path)  # type: ignore[attr-defined]
    elif system_name == "Darwin":
        subprocess.run(["open", str(path)], check=False)
    else:
        subprocess.run(["xdg-open", str(path)], check=False)


def open_with_dialog(path: Path) -> None:
    system_name = platform.system()
    if system_name == "Windows":
        subprocess.run(
            ["rundll32.exe", "shell32.dll,OpenAs_RunDLL", str(path)],
            check=False,
        )
        return
    if system_name == "Darwin":
        subprocess.run(["open", "-a", "TextEdit", str(path)], check=False)
        return
    subprocess.run(["xdg-open", str(path)], check=False)


def find_archive_tool() -> list[str] | None:
    for candidate in ("7z", "7zz", "7za"):
        resolved = shutil.which(candidate)
        if resolved:
            return [resolved]
    return None


def is_archive(path: Path) -> bool:
    return path.suffix.lower() in ARCHIVE_EXTENSIONS and path.is_file()


def build_extraction_preview(
    archives: Iterable[Path],
    options: ExtractionOptions,
) -> list[tuple[Path, Path]]:
    plan: list[tuple[Path, Path]] = []
    for archive in archives:
        destination_root = options.destination_path if options.destination_mode == "custom" else archive.parent
        if destination_root is None:
            destination_root = archive.parent
        destination = destination_root / archive.stem if options.individual_folders else destination_root
        plan.append((archive, destination))
    return plan


def extract_archives(
    archives: Iterable[Path],
    options: ExtractionOptions,
    progress: ProgressCallback,
    log: LogCallback,
) -> None:
    archive_list = list(archives)
    plan = build_extraction_preview(archive_list, options)
    tool = find_archive_tool()
    total = max(len(plan), 1)

    for index, (archive, destination) in enumerate(plan, start=1):
        destination.mkdir(parents=True, exist_ok=True)
        log(f"Descomprimiendo {archive} -> {destination}")

        if archive.suffix.lower() == ".zip":
            with zipfile.ZipFile(archive) as zip_file:
                if not options.overwrite:
                    for member in zip_file.infolist():
                        target = destination / member.filename
                        if target.exists():
                            raise FileExistsError(f"El destino ya existe y sobrescritura no esta permitida: {target}")
                zip_file.extractall(destination)
        else:
            if tool is None:
                raise RuntimeError(
                    "No se encontro 7z/7zz/7za en el sistema para extraer archivos .7z o .rar."
                )
            command = tool + ["x", str(archive), f"-o{destination}"]
            if options.overwrite:
                command.append("-y")
            subprocess.run(command, check=True)

        if options.delete_archives:
            archive.unlink(missing_ok=False)
            log(f"Archivo eliminado tras extraer: {archive}")

        progress(index / total, f"Descomprimiendo {archive.name}")


def detect_chdman(project_root: Path) -> Path | None:
    system_name = get_platform_name()
    candidate_paths = [
        project_root / "third_party" / "chdman" / system_name / ("chdman.exe" if system_name == "windows" else "chdman"),
        project_root / "third_party" / "chdman" / ("chdman.exe" if system_name == "windows" else "chdman"),
    ]

    for candidate in candidate_paths:
        if candidate.exists():
            return candidate

    resolved = shutil.which("chdman")
    return Path(resolved) if resolved else None


def load_chdman_metadata(project_root: Path) -> dict:
    metadata_file = project_root / CHDMAN_METADATA_PATH
    if not metadata_file.exists():
        return {}
    try:
        return json.loads(metadata_file.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return {}


def parse_cue_references(cue_path: Path) -> list[Path]:
    references: list[Path] = [cue_path]
    pattern = re.compile(r'FILE\s+"(.+?)"', re.IGNORECASE)
    try:
        content = cue_path.read_text(encoding="utf-8", errors="ignore")
    except OSError:
        return references
    for match in pattern.finditer(content):
        references.append((cue_path.parent / match.group(1)).resolve())
    return dedupe_paths(references)


def parse_gdi_references(gdi_path: Path) -> list[Path]:
    references: list[Path] = [gdi_path]
    try:
        lines = gdi_path.read_text(encoding="utf-8", errors="ignore").splitlines()
    except OSError:
        return references
    for line in lines[1:]:
        parts = line.strip().split()
        if len(parts) >= 5:
            references.append((gdi_path.parent / parts[4]).resolve())
    return dedupe_paths(references)


def find_cue_for_bin(bin_path: Path) -> Path | None:
    for cue_path in bin_path.parent.glob("*.cue"):
        for reference in parse_cue_references(cue_path):
            if reference.resolve() == bin_path.resolve():
                return cue_path
    return None


def dedupe_paths(paths: Iterable[Path]) -> list[Path]:
    seen: set[Path] = set()
    deduped: list[Path] = []
    for path in paths:
        resolved = path.resolve()
        if resolved not in seen:
            seen.add(resolved)
            deduped.append(path)
    return deduped


def detect_chd_source(path: Path) -> ChdConversionSource | None:
    suffix = path.suffix.lower()
    if suffix in CD_SOURCE_EXTENSIONS:
        required = parse_cue_references(path) if suffix == ".cue" else parse_gdi_references(path) if suffix == ".gdi" else [path]
        return ChdConversionSource(
            command="createcd",
            source_path=path,
            required_paths=required,
            display_extensions=(suffix,),
            container_dir=path.parent,
        )
    if suffix in DVD_SOURCE_EXTENSIONS:
        return ChdConversionSource(
            command="createdvd",
            source_path=path,
            required_paths=[path],
            display_extensions=(suffix,),
            container_dir=path.parent,
        )
    if suffix in PAIRABLE_BIN_EXTENSIONS:
        cue_candidate = path.with_suffix(".cue")
        if cue_candidate.exists():
            return ChdConversionSource(
                command="createcd",
                source_path=cue_candidate,
                required_paths=parse_cue_references(cue_candidate),
                display_extensions=(".bin", ".cue"),
                container_dir=path.parent,
            )
        referenced_cue = find_cue_for_bin(path)
        if referenced_cue is not None:
            return ChdConversionSource(
                command="createcd",
                source_path=referenced_cue,
                required_paths=parse_cue_references(referenced_cue),
                display_extensions=(".bin", ".cue"),
                container_dir=path.parent,
            )
    return None


def gather_chd_sources_from_selection(paths: Iterable[Path]) -> list[ChdConversionSource]:
    sources: list[ChdConversionSource] = []
    seen: set[Path] = set()
    for path in paths:
        if path.is_dir():
            for root, _, filenames in os.walk(path):
                for filename in filenames:
                    candidate = detect_chd_source(Path(root) / filename)
                    if candidate and candidate.source_path.resolve() not in seen:
                        sources.append(candidate)
                        seen.add(candidate.source_path.resolve())
            continue

        candidate = detect_chd_source(path)
        if candidate and candidate.source_path.resolve() not in seen:
            sources.append(candidate)
            seen.add(candidate.source_path.resolve())

    sources.sort(key=lambda item: str(item.source_path).lower())
    return sources


def format_extension_labels(sources: Iterable[ChdConversionSource]) -> str:
    extensions: list[str] = []
    for source in sources:
        for ext in source.display_extensions:
            if ext not in extensions:
                extensions.append(ext)
    return " + ".join(extensions)


def build_chd_output_path(
    source: ChdConversionSource,
    selected_root: Path | None,
    options: ChdConversionOptions,
) -> Path:
    stem = source.source_path.stem
    if options.name_as_container:
        stem = source.container_dir.name

    output_dir = source.source_path.parent
    if options.deposit_to_parent and selected_root is not None:
        try:
            source.source_path.relative_to(selected_root)
        except ValueError:
            pass
        else:
            if source.source_path.parent != selected_root:
                output_dir = selected_root

    return output_dir / f"{stem}.chd"


def run_chd_conversion(
    sources: Iterable[ChdConversionSource],
    options: ChdConversionOptions,
    project_root: Path,
    selected_root: Path | None,
    progress: ProgressCallback,
    log: LogCallback,
) -> None:
    chdman_path = detect_chdman(project_root)
    if chdman_path is None:
        raise RuntimeError(
            "No se encontro chdman. Coloca el binario en third_party/chdman o instalo en el PATH."
        )

    source_list = list(sources)
    total = max(len(source_list), 1)
    touched_directories: set[Path] = set()

    for index, source in enumerate(source_list, start=1):
        output_path = build_chd_output_path(source, selected_root, options)
        output_path.parent.mkdir(parents=True, exist_ok=True)

        command = [
            str(chdman_path),
            source.command,
            "-i",
            str(source.source_path),
            "-o",
            str(output_path),
        ]
        if options.overwrite:
            command.append("-f")

        log("Ejecutando: " + " ".join(command))
        result = subprocess.run(command, check=False, capture_output=True, text=True)
        if result.stdout.strip():
            log(result.stdout.strip())
        if result.returncode != 0:
            error_message = result.stderr.strip() or "chdman devolvio un error."
            raise RuntimeError(error_message)

        if options.delete_originals:
            for original_path in source.required_paths:
                if original_path.exists():
                    if original_path.is_dir():
                        shutil.rmtree(original_path)
                    else:
                        original_path.unlink()
                    log(f"Original eliminado: {original_path}")

        if options.deposit_to_parent and selected_root is not None:
            touched_directories.add(source.source_path.parent)

        progress(index / total, f"Convirtiendo {source.source_path.name}")

    if options.deposit_to_parent and options.delete_original_subfolders:
        for directory in sorted(touched_directories, key=lambda item: len(item.parts), reverse=True):
            if directory == selected_root:
                continue
            if directory.exists() and not any(directory.iterdir()):
                directory.rmdir()
                log(f"Subcarpeta eliminada: {directory}")
