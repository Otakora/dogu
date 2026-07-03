from __future__ import annotations

import argparse
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
TAURI_TARGET_DIR = TAURI_DIR / "target" / "release"
TAURI_BUNDLE_DIR = TAURI_TARGET_DIR / "bundle"
TAURI_CLI = PROJECT_ROOT / "node_modules" / "@tauri-apps" / "cli" / "tauri.js"
FLATPAK_MANIFEST = PROJECT_ROOT / "packaging" / "flatpak" / "io.github.otakora.dogu.yml"
FLATPAK_APP_ID = "io.github.otakora.dogu"
FLATPAK_RUNTIME = "org.freedesktop.Platform//24.08"
FLATPAK_SDK = "org.freedesktop.Sdk//24.08"
FLATPAK_NODE_EXT = "org.freedesktop.Sdk.Extension.node20//24.08"
FLATPAK_RUST_EXT = "org.freedesktop.Sdk.Extension.rust-stable//24.08"


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


def versioned_filename(target: str) -> str:
    version = release_version()
    mapping = {
        "windows": f"dogu-windows-x64-{version}-setup.exe",
        "linux-appimage": f"dogu-linux-x86_64-{version}.AppImage",
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
    if host_os() == "windows":
        program_files = Path(os.environ.get("ProgramFiles", r"C:\Program Files"))
        preferred = program_files / "nodejs" / "node.exe"
        if preferred.exists():
            return preferred
    resolved = shutil.which("node") or shutil.which("nodejs")
    if not resolved:
        raise RuntimeError("No se encontro Node.js. Instala Node 20+ antes de compilar.")
    return Path(resolved)


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
    return env


def ensure_node_modules() -> None:
    env = build_env()
    npm = npm_command()
    lock_file = PROJECT_ROOT / "package-lock.json"
    command = [str(npm), "ci" if lock_file.exists() else "install"]
    run(command, env=env)


def ensure_linux_sidecars_permissions() -> None:
    if host_os() != "linux":
        return
    for binary in (
        PROJECT_ROOT / "third_party" / "chdman" / "linux" / "chdman",
        PROJECT_ROOT / "third_party" / "7zip" / "linux" / "7zz",
    ):
        if binary.exists():
            current = binary.stat().st_mode
            binary.chmod(current | 0o755)


def run_tauri(args: list[str]) -> None:
    env = build_env()
    ensure_node_modules()
    run([str(node_executable()), str(TAURI_CLI), *args], env=env)


def copy_latest(source_pattern: str, target_name: str) -> Path:
    matches = sorted(TAURI_BUNDLE_DIR.glob(source_pattern))
    if not matches:
        raise RuntimeError(f"No se encontro el artefacto esperado: {source_pattern}")
    artifact = matches[-1]
    DIST_DIR.mkdir(parents=True, exist_ok=True)
    target = DIST_DIR / target_name
    shutil.copy2(artifact, target)
    return target


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

    run_tauri(["build", "--bundles", "nsis"])
    return copy_latest("nsis/*-setup.exe", versioned_filename("windows"))


def build_linux_native() -> tuple[Path, Path]:
    if host_os() != "linux":
        raise RuntimeError(
            "El build Linux nativo debe ejecutarse en Linux. Para Windows usa la maquina Windows o CI."
        )

    ensure_linux_sidecars_permissions()
    run_tauri(["build"])
    appimage = copy_latest("appimage/*.AppImage", versioned_filename("linux-appimage"))
    deb = copy_latest("deb/*.deb", versioned_filename("linux-deb"))
    stage_flatpak_input()
    return appimage, deb


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

    build_linux_native()
    for command in flatpak_build_commands():
        run(command)
    return DIST_DIR / versioned_filename("linux-flatpak")


def main() -> None:
    parser = argparse.ArgumentParser(description="Release tooling for Dogu (Tauri edition).")
    subparsers = parser.add_subparsers(dest="command", required=True)
    subparsers.add_parser("build-windows", help="Genera el instalador NSIS de Windows.")
    subparsers.add_parser("build-linux-native", help="Genera AppImage y DEB en Linux.")
    subparsers.add_parser("build-linux-flatpak", help="Genera el bundle Flatpak en Linux.")

    args = parser.parse_args()

    if args.command == "build-windows":
        print(build_windows())
        return
    if args.command == "build-linux-native":
        artifacts = build_linux_native()
        for artifact in artifacts:
            print(artifact)
        return
    if args.command == "build-linux-flatpak":
        print(build_linux_flatpak())
        return


if __name__ == "__main__":
    main()
