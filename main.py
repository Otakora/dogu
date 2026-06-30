from __future__ import annotations


def main() -> None:
    raise SystemExit(
        "La aplicacion ya no arranca desde main.py. "
        "Usa `npm run tauri:dev` para desarrollo o `python tools/release.py ...` para compilar."
    )


if __name__ == "__main__":
    main()
