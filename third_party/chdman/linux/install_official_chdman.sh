#!/usr/bin/env bash
# Builds a portable, statically-linked chdman from the official MAME source.
#
# The resulting binary links SDL2 and libstdc++ statically so it has no
# shared-library dependencies beyond glibc (universally present on any
# Linux system since ~2012 / glibc 2.17+).
#
# Usage:
#   sudo bash install_official_chdman.sh
#
# The finished binary is written to:
#   third_party/chdman/linux/chdman
# Commit that file to make it available to other developers and CI.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORK_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/.build-linux"
MAME_DIR="$WORK_DIR/mame"
SDL2_DIR="$WORK_DIR/SDL2"
OUTPUT_PATH="$SCRIPT_DIR/chdman"

MAME_TAG="mame0288"
SDL2_VERSION="2.30.9"

JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"

# ── 0. Root check ──────────────────────────────────────────
if [ "$(id -u)" -ne 0 ]; then
  echo "Este script necesita root o sudo para instalar dependencias de compilación." >&2
  exit 1
fi

echo "=== Dogu / chdman static build ==="
echo "MAME tag  : $MAME_TAG"
echo "SDL2      : $SDL2_VERSION (static)"
echo "Output    : $OUTPUT_PATH"
echo "Jobs      : $JOBS"
echo ""

# ── 1. Build-time dependencies (compile only, not required at runtime) ──────
apt-get update -qq
DEBIAN_FRONTEND=noninteractive apt-get install -y \
  build-essential \
  cmake \
  git \
  wget \
  pkg-config \
  python3 \
  libfontconfig1-dev \
  libasound2-dev \
  libgl1-mesa-dev \
  libegl-dev \
  libx11-dev \
  libxext-dev

mkdir -p "$WORK_DIR"

# ── 2. Build SDL2 from source — static, minimal feature set ─────────────────
# chdman only uses SDL2 for platform abstraction (timers, threads, filesystem).
# Disable all display/audio/input subsystems to eliminate their runtime deps.

SDL2_SRC="$WORK_DIR/SDL2-${SDL2_VERSION}"
SDL2_BUILD="$SDL2_SRC/build-static"

if [ ! -d "$SDL2_SRC" ]; then
  echo "--- Descargando SDL2 ${SDL2_VERSION}..."
  wget -q -O "$WORK_DIR/SDL2.tar.gz" \
    "https://github.com/libsdl-org/SDL/releases/download/release-${SDL2_VERSION}/SDL2-${SDL2_VERSION}.tar.gz"
  tar -xf "$WORK_DIR/SDL2.tar.gz" -C "$WORK_DIR"
fi

echo "--- Compilando SDL2 (static, minimal)..."
mkdir -p "$SDL2_BUILD"
cmake -S "$SDL2_SRC" -B "$SDL2_BUILD" \
  -DCMAKE_BUILD_TYPE=Release \
  -DSDL_STATIC=ON  \
  -DSDL_SHARED=OFF \
  -DSDL_TEST=OFF   \
  -DVIDEO_X11=OFF  -DVIDEO_WAYLAND=OFF -DVIDEO_OPENGL=OFF \
  -DVIDEO_OPENGLES=OFF -DVIDEO_VULKAN=OFF -DVIDEO_DUMMY=OFF \
  -DAUDIO_ALSA=OFF -DAUDIO_PULSEAUDIO=OFF -DAUDIO_JACK=OFF \
  -DAUDIO_SNDIO=OFF -DAUDIO_NAS=OFF -DAUDIO_OSS=OFF \
  -DAUDIO_DUMMY=OFF \
  -DJOYSTICK_LINUX=OFF -DJOYSTICK_HIDAPI=OFF \
  -DHAPTIC=OFF -DSENSOR=OFF -DPOWER=OFF \
  -DFILESYSTEM=ON -DTHREADS=ON -DTIMERS=ON \
  -DCMAKE_POSITION_INDEPENDENT_CODE=ON
cmake --build "$SDL2_BUILD" -j"$JOBS"

SDL2_STATIC_LIB="$SDL2_BUILD/libSDL2.a"
if [ ! -f "$SDL2_STATIC_LIB" ]; then
  echo "ERROR: no se generó libSDL2.a en $SDL2_BUILD" >&2
  exit 1
fi
echo "--- libSDL2.a lista: $SDL2_STATIC_LIB"

# ── 3. Clone / update MAME source ────────────────────────────────────────────
if [ ! -d "$MAME_DIR/.git" ]; then
  echo "--- Clonando MAME ($MAME_TAG)..."
  git clone -b "$MAME_TAG" --depth 1 https://github.com/mamedev/mame.git "$MAME_DIR"
else
  echo "--- Actualizando MAME ($MAME_TAG)..."
  git -C "$MAME_DIR" fetch --depth 1 origin "$MAME_TAG"
  git -C "$MAME_DIR" checkout "$MAME_TAG"
fi
git config --global --add safe.directory "$MAME_DIR" 2>/dev/null || true

# ── 4. Build MAME tools (chdman only) ────────────────────────────────────────
echo "--- Compilando herramientas MAME (TOOLS=1, EMULATOR=0)..."
make -C "$MAME_DIR" REGENIE=1 TOOLS=1 EMULATOR=0 USE_QTDEBUG=0 -j"$JOBS"

# ── 5. Link chdman against static SDL2 ────────────────────────────────────────
echo "--- Enlazando chdman con SDL2 estático..."

OBJ_ROOT="$MAME_DIR/build/linux_gcc"
BIN_DIR="$OBJ_ROOT/bin/x64/Release"
OBJ_DIR="$OBJ_ROOT/obj/x64/Release"

g++ -o "$OUTPUT_PATH" \
  "$OBJ_DIR/src/tools/chdman.o" \
  "$OBJ_DIR/generated/version.o" \
  "$BIN_DIR/libutils.a" \
  "$BIN_DIR/libexpat.a" \
  "$BIN_DIR/lib7z.a" \
  "$BIN_DIR/libocore_sdl.a" \
  "$BIN_DIR/libzlib.a" \
  "$BIN_DIR/libzstd.a" \
  "$BIN_DIR/libflac.a" \
  "$BIN_DIR/libutf8proc.a" \
  "$SDL2_STATIC_LIB" \
  -ldl -lrt -lm -lpthread -lutil \
  -static-libstdc++ -static-libgcc

chmod +x "$OUTPUT_PATH"

echo ""
echo "=== Build finalizado ==="
echo "Binario : $OUTPUT_PATH"
echo ""

# Show runtime dependencies — should only list linux-vdso, libpthread, libdl,
# libc, libm, librt, libutil (all part of glibc, present on every Linux system).
echo "--- Dependencias de runtime (ldd):"
ldd "$OUTPUT_PATH" || true
echo ""
echo "Comprueba que no aparece libSDL2 ni libstdc++ en la lista anterior."
echo "Si aparecen, el enlace estático no funcionó — revisa los flags."
echo ""
echo "Cuando el binario sea correcto, haz commit del fichero:"
echo "  git add third_party/chdman/linux/chdman && git commit -m 'chore: update static chdman Linux binary'"
