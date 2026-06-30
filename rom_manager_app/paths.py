from __future__ import annotations

import platform
import sys
from pathlib import Path


def is_frozen() -> bool:
    return bool(getattr(sys, "frozen", False))


def get_project_root() -> Path:
    if is_frozen():
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent.parent


def get_platform_name() -> str:
    system_name = platform.system().lower()
    if system_name.startswith("win"):
        return "windows"
    if system_name.startswith("linux"):
        return "linux"
    return system_name
