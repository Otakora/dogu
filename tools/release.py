from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
import platform
import shutil
import subprocess
import sys
import tomllib
from pathlib import Path


PROJECT_ROOT = Path(__file__).resolve().parent.parent
DIST_DIR = PROJECT_ROOT / "dist"
BUILD_DIR = PROJECT_ROOT / "build"
TAURI_DIR = PROJECT_ROOT / "src-tauri"
TAURI_CONF = TAURI_DIR / "tauri.conf.json"
TAURI_TARGET_DIR = TAURI_DIR / "target" / "release"
TAURI_BUNDLE_DIR = TAURI_TARGET_DIR / "bundle"
TAURI_CLI = PROJECT_ROOT / "node_modules" / "@tauri-apps" / "cli" / "tauri.js"
CHANGELOG = PROJECT_ROOT / "CHANGELOG.md"
FLATPAK_MANIFEST = PROJECT_ROOT / "packaging" / "flatpak" / "io.github.otakora.dogu.yml"
FLATPAK_APP_ID = "io.github.otakora.dogu"
FLATPAK_RUNTIME = "org.freedesktop.Platform//24.08"
FLATPAK_SDK = "org.freedesktop.Sdk//24.08"
FLATPAK_NODE_EXT = "org.freedesktop.Sdk.Extension.node20//24.08"
FLATPAK_RUST_EXT = "org.freedesktop.Sdk.Extension.rust-stable//24.08"
GIT_LFS_POINTER_PREFIX = b"version https://git-lfs.github.com/spec/v1"
BUNDLED_SIDECARS = (
    PROJECT_ROOT / "third_party" / "7zip" / "linux" / "7zz",
    PROJECT_ROOT / "third_party" / "7zip" / "windows" / "7z.dll",
    PROJECT_ROOT / "third_party" / "7zip" / "windows" / "7z.exe",
    PROJECT_ROOT / "third_party" / "7zip" / "windows" / "7za.dll",
    PROJECT_ROOT / "third_party" / "7zip" / "windows" / "7za.exe",
    PROJECT_ROOT / "third_party" / "chdman" / "linux" / "chdman",
    PROJECT_ROOT / "third_party" / "chdman" / "windows" / "chdman.exe",
    PROJECT_ROOT / "third_party" / "dolphin-tool" / "linux" / "DolphinTool",
    PROJECT_ROOT / "third_party" / "dolphin-tool" / "windows" / "DolphinTool.exe",
)


def run(command: list[str], cwd: Path | None = None, env: dict[str, str] | None = None) -> None:
    subprocess.run(command, cwd=cwd or PROJECT_ROOT, env=env, check=True)


def release_version() -> str:
    cargo_toml = TAURI_DIR / "Cargo.toml"
    if cargo_toml.exists():
        data = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))
        version = data.get("package", {}).get("version")
        if isinstance(version, str) and version.strip():
            return version.strip()

    package_json = PROJECT_ROOT / "package.json"
    if package_json.exists():
        data = json.loads(package_json.read_text(encoding="utf-8"))
        version = data.get("version")
        if isinstance(version, str) and version.strip():
            return version.strip()

    raise RuntimeError("No se pudo determinar la version actual del proyecto.")


def verify_version_metadata() -> str:
    cargo_toml = TAURI_DIR / "Cargo.toml"
    package_json = PROJECT_ROOT / "package.json"

    cargo_version = tomllib.loads(cargo_toml.read_text(encoding="utf-8"))["package"]["version"]
    package_version = json.loads(package_json.read_text(encoding="utf-8"))["version"]
    tauri_version = json.loads(TAURI_CONF.read_text(encoding="utf-8"))["version"]

    versions = {
        "src-tauri/Cargo.toml": cargo_version,
        "package.json": package_version,
        "src-tauri/tauri.conf.json": tauri_version,
    }
    unique = set(versions.values())
    if len(unique) != 1:
        details = ", ".join(f"{path}={version!r}" for path, version in versions.items())
        raise RuntimeError(f"Version metadata mismatch: {details}")

    return cargo_version


def versioned_filename(target: str, version: str | None = None) -> str:
    version = version or release_version()
    mapping = {
        "windows": f"dogu-windows-x64-{version}-setup.exe",
        "windows-sig": f"dogu-windows-x64-{version}-setup.exe.sig",
        "linux-appimage": f"dogu-linux-x86_64-{version}.AppImage",
        "linux-appimage-sig": f"dogu-linux-x86_64-{version}.AppImage.sig",
        "linux-deb": f"dogu-linux-x86_64-{version}.deb",
        "linux-flatpak": f"dogu-linux-x86_64-{version}.flatpak",
    }
    if target not in mapping:
        raise RuntimeError(f"Target de release desconocido: {target}")
    return mapping[target]


def host_os() -> str:
    system_name = platform.system().lower()
    if system_name.startswith("win"):
        return "windows"
    if system_name.startswith("linux"):
        return "linux"
    return system_name


def node_executable() -> Path:
    resolved = shutil.which("node") or shutil.which("nodejs")
    if resolved:
        return Path(resolved)
    if host_os() == "windows":
        program_files = Path(os.environ.get("ProgramFiles", r"C:\Program Files"))
        preferred = program_files / "nodejs" / "node.exe"
        if preferred.exists():
            return preferred
    raise RuntimeError("No se encontro Node.js. Instala Node 20.19+ o 22.12+ antes de compilar.")


def npm_command() -> Path:
    node_path = node_executable()
    if host_os() == "windows":
        candidate = node_path.with_name("npm.cmd")
        if candidate.exists():
            return candidate
    resolved = shutil.which("npm")
    if not resolved:
        raise RuntimeError("No se encontro npm. Instala Node.js con npm habilitado.")
    return Path(resolved)


def build_env() -> dict[str, str]:
    env = os.environ.copy()
    node_path = node_executable()
    env["PATH"] = str(node_path.parent) + os.pathsep + env.get("PATH", "")
    if host_os() == "linux":
        env.setdefault("APPIMAGE_EXTRACT_AND_RUN", "1")
        env.setdefault("NO_STRIP", "true")
    return env


def updater_public_key_from_env(env: dict[str, str]) -> str:
    public_key = env.get("DOGU_UPDATER_PUBLIC_KEY", "").strip()
    if not public_key:
        raise RuntimeError(
            "Falta DOGU_UPDATER_PUBLIC_KEY. Tauri necesita la clave publica updater "
            "en la configuracion para inicializar el plugin."
        )
    return public_key


def tauri_updater_config(public_key: str) -> str:
    return json.dumps(
        {
            "plugins": {
                "updater": {
                    "pubkey": public_key,
                    "windows": {
                        "installMode": "passive",
                    },
                },
            },
        },
        separators=(",", ":"),
    )


def ensure_node_modules() -> None:
    env = build_env()
    npm = npm_command()
    lock_file = PROJECT_ROOT / "package-lock.json"
    command = [str(npm), "ci" if lock_file.exists() else "install"]
    run(command, env=env)


def ensure_bundled_sidecars_materialized() -> None:
    missing = [path for path in BUNDLED_SIDECARS if not path.exists()]
    if missing:
        names = ", ".join(str(path.relative_to(PROJECT_ROOT)) for path in missing)
        raise RuntimeError(f"Faltan sidecars integrados necesarios para la release: {names}")

    pointers = []
    for path in BUNDLED_SIDECARS:
        with path.open("rb") as handle:
            head = handle.read(len(GIT_LFS_POINTER_PREFIX))
        if head == GIT_LFS_POINTER_PREFIX:
            pointers.append(path)

    if pointers:
        names = ", ".join(str(path.relative_to(PROJECT_ROOT)) for path in pointers)
        raise RuntimeError(
            "Hay sidecars que son punteros de Git LFS en vez de binarios reales: "
            f"{names}. Ejecuta `git lfs pull` o usa `actions/checkout` con `lfs: true`."
        )


def ensure_linux_sidecars_permissions() -> None:
    if host_os() != "linux":
        return
    for binary in (
        PROJECT_ROOT / "third_party" / "chdman" / "linux" / "chdman",
        PROJECT_ROOT / "third_party" / "7zip" / "linux" / "7zz",
        PROJECT_ROOT / "third_party" / "dolphin-tool" / "linux" / "DolphinTool",
    ):
        if binary.exists():
            current = binary.stat().st_mode
            binary.chmod(current | 0o755)


def ensure_windows_rar_extraction_sidecars() -> None:
    if host_os() != "windows":
        return
    required = (
        PROJECT_ROOT / "third_party" / "7zip" / "windows" / "7z.exe",
        PROJECT_ROOT / "third_party" / "7zip" / "windows" / "7z.dll",
    )
    missing = [path for path in required if not path.exists()]
    if missing:
        names = ", ".join(str(path.relative_to(PROJECT_ROOT)) for path in missing)
        raise RuntimeError(
            "Faltan sidecars necesarios para extraccion RAR nativa en Windows: "
            f"{names}"
        )


def run_tauri(args: list[str]) -> None:
    env = build_env()
    public_key = updater_public_key_from_env(env)
    ensure_bundled_sidecars_materialized()
    ensure_node_modules()
    command = [str(node_executable()), str(TAURI_CLI), *args[:1], "--config", tauri_updater_config(public_key), *args[1:]]
    run(command, env=env)


def copy_latest(source_pattern: str, target_name: str) -> Path:
    matches = sorted(TAURI_BUNDLE_DIR.glob(source_pattern))
    if not matches:
        raise RuntimeError(f"No se encontro el artefacto esperado: {source_pattern}")
    artifact = matches[-1]
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    target = DIST_DIR / target_name
    shutil.copy2(artifact, target)
    return target


def copy_signature_for(artifact: Path, target_name: str) -> Path:
    source = artifact.with_name(f"{artifact.name}.sig")
    if not source.exists():
        raise RuntimeError(f"No se encontro la firma updater esperada: {source}")
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    target = DIST_DIR / target_name
    shutil.copy2(source, target)
    return target


def copy_latest_signature(source_pattern: str, target_name: str) -> Path:
    matches = sorted(TAURI_BUNDLE_DIR.glob(source_pattern))
    if not matches:
        raise RuntimeError(f"No se encontro el artefacto esperado para firmar: {source_pattern}")
    return copy_signature_for(matches[-1], target_name)


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def updater_signature(path: Path) -> str:
    signature_path = path.with_name(f"{path.name}.sig")
    if not signature_path.exists():
        raise RuntimeError(f"Falta la firma updater para {path.name}: {signature_path.name}")
    return signature_path.read_text(encoding="utf-8").strip()


def extract_changelog_section(version: str) -> str:
    if not CHANGELOG.exists():
        raise RuntimeError("Falta CHANGELOG.md; las releases deben incluir notas bilingues resumidas.")

    lines = CHANGELOG.read_text(encoding="utf-8").splitlines()
    headings = [f"## {version}"]
    if "-" in version:
        headings.append(f"## {version.split('-', 1)[0]}")
    start = None
    matched_heading = None
    for heading in headings:
        for index, line in enumerate(lines):
            if line.strip() == heading:
                start = index + 1
                matched_heading = heading
                break
        if start is not None:
            break
    if start is None:
        expected = " o ".join(f"'{heading}'" for heading in headings)
        raise RuntimeError(f"CHANGELOG.md no contiene una seccion {expected}.")

    end = len(lines)
    for index in range(start, len(lines)):
        if lines[index].startswith("## "):
            end = index
            break

    body = "\n".join(lines[start:end]).strip()
    if not body:
        raise RuntimeError(f"La seccion '{matched_heading}' de CHANGELOG.md esta vacia.")
    return body


def write_release_notes(version: str, output: Path) -> Path:
    output.parent.mkdir(parents=True, exist_ok=True)
    body = extract_changelog_section(version)
    output.write_text(f"# Dogu {version}\n\n{body}\n", encoding="utf-8")
    return output


def updater_notes_from_file(path: Path) -> str:
    notes = path.read_text(encoding="utf-8").strip()
    lines = notes.splitlines()
    if lines and lines[0].startswith("# Dogu "):
        return "\n".join(lines[1:]).strip()
    return notes


def write_updater_manifest(channel: str, version: str, base_url: str, output: Path, notes: str = "", notes_file: Path | None = None) -> Path:
    if channel not in {"stable", "beta"}:
        raise RuntimeError(f"Canal updater desconocido: {channel}")

    artifact_root = output.parent
    windows = artifact_root / versioned_filename("windows", version)
    appimage = artifact_root / versioned_filename("linux-appimage", version)
    missing = [path.name for path in (windows, appimage) if not path.exists()]
    if missing:
        raise RuntimeError(f"Faltan artefactos para el updater {channel}: {', '.join(missing)}")

    if notes_file is not None:
        notes = updater_notes_from_file(notes_file)

    manifest = {
        "version": version,
        "notes": notes,
        "pub_date": dt.datetime.now(dt.timezone.utc).isoformat(timespec="seconds"),
        "platforms": {
            "windows-x86_64": {
                "signature": updater_signature(windows),
                "url": f"{base_url.rstrip('/')}/{windows.name}",
            },
            "linux-x86_64": {
                "signature": updater_signature(appimage),
                "url": f"{base_url.rstrip('/')}/{appimage.name}",
            },
        },
    }
    output.write_text(json.dumps(manifest, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return output


def write_winget_manifest(version: str, installer_url: str, output: Path) -> Path:
    installer_name = versioned_filename("windows", version)
    installer_path = output.parent / installer_name
    if not installer_path.exists():
        raise RuntimeError(f"Falta el instalador Windows para winget: {installer_path}")

    manifest = f"""PackageIdentifier: Otakora.Dogu
PackageVersion: {version}
PackageLocale: en-US
Publisher: Otakora
PublisherUrl: https://github.com/Otakora
PackageName: Dogu
PackageUrl: https://github.com/Otakora/dogu
License: GPL-2.0-or-later
LicenseUrl: https://github.com/Otakora/dogu/blob/main/LICENSE
ShortDescription: A practical desktop file workbench for local files, remotes and queued operations.
InstallerType: nullsoft
InstallModes:
- silent
- silentWithProgress
UpgradeBehavior: install
ReleaseDate: {dt.date.today().isoformat()}
Installers:
- Architecture: x64
  InstallerUrl: {installer_url.rstrip('/')}/{installer_name}
  InstallerSha256: {sha256_file(installer_path)}
ManifestType: singleton
ManifestVersion: 1.12.0
"""
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(manifest, encoding="utf-8")
    return output


def stage_flatpak_input() -> Path:
    if host_os() != "linux":
        raise RuntimeError("El staging para Flatpak solo esta soportado en Linux.")
    staged_root = BUILD_DIR / "flatpak-input"
    if staged_root.exists():
        shutil.rmtree(staged_root)
    staged_root.mkdir(parents=True, exist_ok=True)

    binary_source = TAURI_TARGET_DIR / "dogu"
    if not binary_source.exists():
        raise RuntimeError("No se encontro el binario Linux release de Tauri para preparar Flatpak.")
    shutil.copy2(binary_source, staged_root / "dogu")

    bundled_third_party = staged_root / "third_party"
    shutil.copytree(PROJECT_ROOT / "third_party", bundled_third_party, dirs_exist_ok=True)
    return staged_root


def build_windows() -> Path:
    if host_os() != "windows":
        raise RuntimeError(
            "El build de Windows debe ejecutarse en Windows. Para obtener el artefacto desde Linux usa CI."
        )

    ensure_windows_rar_extraction_sidecars()
    run_tauri(["build", "--bundles", "nsis"])
    installer = copy_latest("nsis/*-setup.exe", versioned_filename("windows"))
    copy_latest_signature("nsis/*-setup.exe", versioned_filename("windows-sig"))
    return installer


def build_linux_native() -> tuple[Path, Path]:
    if host_os() != "linux":
        raise RuntimeError(
            "El build Linux nativo debe ejecutarse en Linux. Para Windows usa la maquina Windows o CI."
        )

    ensure_linux_sidecars_permissions()
    run_tauri(["build", "-vv", "--bundles", "deb,appimage"])
    appimage = copy_latest("appimage/*.AppImage", versioned_filename("linux-appimage"))
    copy_latest_signature("appimage/*.AppImage", versioned_filename("linux-appimage-sig"))
    deb = copy_latest("deb/*.deb", versioned_filename("linux-deb"))
    stage_flatpak_input()
    return appimage, deb


def build_linux_deb() -> Path:
    if host_os() != "linux":
        raise RuntimeError(
            "El build Linux DEB debe ejecutarse en Linux. Para Windows usa la maquina Windows o CI."
        )

    ensure_linux_sidecars_permissions()
    run_tauri(["build", "--bundles", "deb"])
    return copy_latest("deb/*.deb", versioned_filename("linux-deb"))


def flatpak_build_commands() -> list[list[str]]:
    build_root = BUILD_DIR / "flatpak-builder"
    repo_dir = DIST_DIR / "flatpak-repo"
    bundle_path = DIST_DIR / versioned_filename("linux-flatpak")
    return [
        ["flatpak", "remote-add", "--user", "--if-not-exists", "flathub", "https://flathub.org/repo/flathub.flatpakrepo"],
        ["flatpak", "install", "--user", "-y", "flathub", FLATPAK_RUNTIME, FLATPAK_SDK, FLATPAK_NODE_EXT, FLATPAK_RUST_EXT],
        ["flatpak-builder", "--force-clean", "--repo", str(repo_dir), str(build_root), str(FLATPAK_MANIFEST)],
        ["flatpak", "build-bundle", str(repo_dir), str(bundle_path), FLATPAK_APP_ID],
    ]


def build_linux_flatpak() -> Path:
    if host_os() != "linux":
        raise RuntimeError("El build local de Flatpak solo esta soportado en Linux.")

    if shutil.which("flatpak") is None or shutil.which("flatpak-builder") is None:
        raise RuntimeError("Instala flatpak y flatpak-builder antes de generar el bundle Flatpak.")

    ensure_linux_sidecars_permissions()
    run_tauri(["build", "--no-bundle"])
    stage_flatpak_input()
    for command in flatpak_build_commands():
        run(command)
    return DIST_DIR / versioned_filename("linux-flatpak")


def main() -> None:
    parser = argparse.ArgumentParser(description="Release tooling for Dogu (Tauri edition).")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("build-windows", help="Genera el instalador NSIS de Windows.")
    subparsers.add_parser("build-linux-deb", help="Genera solo el paquete DEB de Linux.")
    subparsers.add_parser("build-linux-native", help="Genera AppImage y DEB en Linux.")
    subparsers.add_parser("build-linux-flatpak", help="Genera el bundle Flatpak en Linux.")
    subparsers.add_parser("verify-version", help="Verifica que todos los metadatos de version coinciden.")

    notes_parser = subparsers.add_parser("write-release-notes", help="Extrae notas bilingues de CHANGELOG.md.")
    notes_parser.add_argument("--version", default=release_version())
    notes_parser.add_argument("--output", required=True, type=Path)

    updater_parser = subparsers.add_parser("write-updater-manifest", help="Genera latest.json/latest-beta.json para Tauri updater.")
    updater_parser.add_argument("--channel", choices=("stable", "beta"), required=True)
    updater_parser.add_argument("--version", default=release_version())
    updater_parser.add_argument("--base-url", required=True)
    updater_parser.add_argument("--output", required=True, type=Path)
    updater_parser.add_argument("--notes", default="")
    updater_parser.add_argument("--notes-file", type=Path)

    winget_parser = subparsers.add_parser("write-winget-manifest", help="Genera el manifest winget Otakora.Dogu.")
    winget_parser.add_argument("--version", default=release_version())
    winget_parser.add_argument("--installer-url", required=True)
    winget_parser.add_argument("--output", required=True, type=Path)

    args = parser.parse_args()

    if args.command == "verify-version":
        print(verify_version_metadata())
        return
    if args.command == "build-windows":
        print(build_windows())
        return
    if args.command == "build-linux-deb":
        print(build_linux_deb())
        return
    if args.command == "build-linux-native":
        artifacts = build_linux_native()
        for artifact in artifacts:
            print(artifact)
        return
    if args.command == "build-linux-flatpak":
        print(build_linux_flatpak())
        return
    if args.command == "write-release-notes":
        print(write_release_notes(args.version, args.output))
        return
    if args.command == "write-updater-manifest":
        print(write_updater_manifest(args.channel, args.version, args.base_url, args.output, args.notes, args.notes_file))
        return
    if args.command == "write-winget-manifest":
        print(write_winget_manifest(args.version, args.installer_url, args.output))
        return


if __name__ == "__main__":
    main()
