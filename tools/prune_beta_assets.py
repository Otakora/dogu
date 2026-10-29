"""Remove obsolete versioned Dogu assets from the rolling beta release."""
from __future__ import annotations

import json
import re
import subprocess
import sys


ASSET = re.compile(
    r"^dogu-(?:windows-x64-(?P<w>\d+\.\d+\.\d+-beta\.\d+)-setup\.exe(?:\.sig)?"
    r"|linux-x86_64-(?P<l>\d+\.\d+\.\d+-beta\.\d+)\.(?:AppImage(?:\.sig)?|deb|flatpak))$"
)


def obsolete_assets(names: list[str], version: str) -> list[str]:
    expected = {
        f"dogu-windows-x64-{version}-setup.exe",
        f"dogu-windows-x64-{version}-setup.exe.sig",
        f"dogu-linux-x86_64-{version}.AppImage",
        f"dogu-linux-x86_64-{version}.AppImage.sig",
        f"dogu-linux-x86_64-{version}.deb",
        f"dogu-linux-x86_64-{version}.flatpak",
        "latest-beta.json",
        "SHA256SUMS.txt",
    }
    missing = expected.difference(names)
    if missing:
        raise ValueError(f"Current beta assets missing; refusing cleanup: {sorted(missing)}")
    obsolete = []
    for name in names:
        match = ASSET.fullmatch(name)
        if match and (match.group("w") or match.group("l")) != version:
            obsolete.append(name)
    return obsolete


def main() -> None:
    if len(sys.argv) != 3 or not re.fullmatch(r"\d+\.\d+\.\d+-beta\.\d+", sys.argv[2]):
        raise SystemExit("Usage: prune_beta_assets.py OWNER/REPO X.Y.Z-beta.N")
    repo, version = sys.argv[1:]
    raw = subprocess.check_output(
        ["gh", "release", "view", "beta", "--repo", repo, "--json", "isPrerelease,assets"],
        text=True,
    )
    release = json.loads(raw)
    if not release["isPrerelease"]:
        raise SystemExit("Rolling beta release is not marked prerelease; refusing cleanup.")
    for name in obsolete_assets([asset["name"] for asset in release["assets"]], version):
        subprocess.run(["gh", "release", "delete-asset", "beta", name, "--repo", repo, "--yes"], check=True)
        print(f"Removed obsolete beta asset: {name}")


if __name__ == "__main__":
    main()
