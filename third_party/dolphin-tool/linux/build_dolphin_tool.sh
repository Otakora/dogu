#!/usr/bin/env bash
# Builds DolphinTool (the `dolphin-tool` CLI) from the official Dolphin source
# and drops it at third_party/dolphin-tool/linux/DolphinTool.
#
# DolphinTool is GPL-2.0+ (part of the Dolphin emulator). Building and
# redistributing it is allowed as long as the corresponding source is offered —
# see third_party/THIRD-PARTY-LICENSES.md. This mirrors the chdman build script.
#
# Usage:
#   sudo bash build_dolphin_tool.sh
#
# Output:
#   third_party/dolphin-tool/linux/DolphinTool
#
# Note: unlike chdman, DolphinTool links a chunk of Dolphin core (via uicommon),
# so a fully-static binary is not practical. Build on an older-glibc distro for
# broad compatibility and check `ldd` at the end for any heavy runtime deps.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/.build-linux"
DOLPHIN_DIR="$WORK_DIR/dolphin"
OUTPUT_PATH="$SCRIPT_DIR/DolphinTool"

# Pinned Dolphin release tag. Update to match the version you want to ship, then
# also update third_party/dolphin-tool/metadata.json.
DOLPHIN_TAG="${DOLPHIN_TAG:-2606}"

JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"

if [ "$(id -u)" -ne 0 ]; then
  echo "Este script necesita root o sudo para instalar dependencias de compilación." >&2
  exit 1
fi

echo "=== Dogu / DolphinTool build ==="
echo "Dolphin tag : $DOLPHIN_TAG"
echo "Output      : $OUTPUT_PATH"
echo "Jobs        : $JOBS"
echo ""

# ── 1. Build-time dependencies ───────────────────────────────────────────────
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y \
  build-essential \
  cmake \
  ninja-build \
  git \
  pkg-config \
  python3 \
  libevdev-dev \
  libudev-dev \
  libcurl4-openssl-dev \
  libsystemd-dev

mkdir -p "$WORK_DIR"

# ── 2. Clone Dolphin (recursive submodules for bundled Externals) ─────────────
if [ ! -d "$DOLPHIN_DIR/.git" ]; then
  echo "--- Clonando Dolphin ($DOLPHIN_TAG)..."
  git clone --depth 1 --recurse-submodules --shallow-submodules \
    -b "$DOLPHIN_TAG" https://github.com/dolphin-emu/dolphin.git "$DOLPHIN_DIR"
else
  echo "--- Reutilizando clon existente en $DOLPHIN_DIR"
  git -C "$DOLPHIN_DIR" fetch --depth 1 origin "$DOLPHIN_TAG"
  git -C "$DOLPHIN_DIR" checkout "$DOLPHIN_TAG"
  git -C "$DOLPHIN_DIR" submodule update --init --recursive --depth 1
fi
git config --global --add safe.directory "$DOLPHIN_DIR" 2>/dev/null || true

# ── 3. Configure — no GUI, no extras, just the CLI tool ──────────────────────
BUILD_DIR="$DOLPHIN_DIR/build"
echo "--- Configurando (CMake, sin Qt/GUI)..."
cmake -S "$DOLPHIN_DIR" -B "$BUILD_DIR" -G Ninja \
  -DCMAKE_BUILD_TYPE=Release \
  -DENABLE_QT=OFF \
  -DENABLE_TESTS=OFF \
  -DENABLE_ANALYTICS=OFF \
  -DENABLE_AUTOUPDATE=OFF \
  -DUSE_DISCORD_PRESENCE=OFF \
  -DENABLE_CLI_TOOL=ON

# ── 4. Build only the dolphin-tool target ────────────────────────────────────
echo "--- Compilando target dolphin-tool..."
cmake --build "$BUILD_DIR" --target dolphin-tool -j"$JOBS"

# ── 5. Locate and copy the binary ────────────────────────────────────────────
BUILT="$(find "$BUILD_DIR" -type f -name 'dolphin-tool' -perm -u+x 2>/dev/null | head -n1)"
if [ -z "$BUILT" ]; then
  echo "ERROR: no se encontró el binario dolphin-tool en $BUILD_DIR" >&2
  exit 1
fi
cp "$BUILT" "$OUTPUT_PATH"
chmod +x "$OUTPUT_PATH"

echo ""
echo "=== Build finalizado ==="
echo "Binario : $OUTPUT_PATH"
echo ""
echo "--- Dependencias de runtime (ldd):"
ldd "$OUTPUT_PATH" || true
echo ""
echo "Comprueba el binario:  $OUTPUT_PATH convert --help"
echo "Cuando sea correcto, haz commit del fichero:"
echo "  git add third_party/dolphin-tool/linux/DolphinTool && git commit -m 'chore: bundle DolphinTool Linux binary'"
