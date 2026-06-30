#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
CHDMAN_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
WORK_DIR="$CHDMAN_ROOT/.build-linux"
SOURCE_DIR="$WORK_DIR/mame0288"
OUTPUT_PATH="$SCRIPT_DIR/chdman"
RELEASE_TAG="mame0288"
SOURCE_URL="https://github.com/mamedev/mame.git"
JOBS="$(getconf _NPROCESSORS_ONLN 2>/dev/null || echo 2)"

echo "Preparando build oficial de chdman para Linux en: $OUTPUT_PATH"
echo "Fuente oficial: $SOURCE_URL ($RELEASE_TAG)"

if [ "$(id -u)" -ne 0 ]; then
  echo "Este script necesita root o sudo para instalar dependencias." >&2
  exit 1
fi

apt-get update
DEBIAN_FRONTEND=noninteractive apt-get install -y \
  build-essential \
  git \
  pkg-config \
  python3 \
  libsdl2-dev \
  libfontconfig1-dev \
  libasound2-dev \
  libgl1-mesa-dev \
  libegl-dev

mkdir -p "$WORK_DIR"

if [ ! -d "$SOURCE_DIR/.git" ]; then
  git clone -b "$RELEASE_TAG" --depth 1 "$SOURCE_URL" "$SOURCE_DIR"
else
  git -C "$SOURCE_DIR" fetch --depth 1 origin "$RELEASE_TAG"
  git -C "$SOURCE_DIR" checkout "$RELEASE_TAG"
fi

cd "$SOURCE_DIR"
git config --global --add safe.directory "$SOURCE_DIR" || true

echo "Compilando desde la fuente oficial de MAME."
echo "La documentacion oficial indica compilar con TOOLS=1 para incluir chdman."
echo "El build Linux de esta integracion usa herramientas y librerias minimas, sin el emulador completo."

make REGENIE=1 TOOLS=1 EMULATOR=0 USE_QTDEBUG=0 -j"$JOBS"

cd "$SOURCE_DIR/build/projects/sdl/mame/gmake-linux"
make -f ocore_sdl.make config=release64 -j"$JOBS"
make -f zlib.make config=release64 -j"$JOBS"
make -f zstd.make config=release64 -j"$JOBS"
make -f flac.make config=release64 -j"$JOBS"
make -f utf8proc.make config=release64 -j"$JOBS"

mkdir -p "$SOURCE_DIR/build/projects/linux_gcc/bin/x64/Release"
cp -af "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/." "$SOURCE_DIR/build/projects/linux_gcc/bin/x64/Release/"

g++ -o "$OUTPUT_PATH" \
  "$SOURCE_DIR/build/linux_gcc/obj/x64/Release/src/tools/chdman.o" \
  "$SOURCE_DIR/build/linux_gcc/obj/x64/Release/generated/version.o" \
  "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/libutils.a" \
  "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/libexpat.a" \
  "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/lib7z.a" \
  "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/libocore_sdl.a" \
  "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/libzlib.a" \
  "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/libzstd.a" \
  "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/libflac.a" \
  "$SOURCE_DIR/build/linux_gcc/bin/x64/Release/libutf8proc.a" \
  -ldl -lrt -lm -lpthread -lutil $(pkg-config --libs sdl2)

chmod +x "$OUTPUT_PATH"

echo "chdman oficial copiado a: $OUTPUT_PATH"
