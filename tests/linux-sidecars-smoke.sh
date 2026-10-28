#!/usr/bin/env bash
# Exercise the bundled Linux binaries with disposable local data.
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
work="$(mktemp -d)"
trap 'rm -rf -- "$work"' EXIT
cp "$root/third_party/7zip/linux/7zz" "$work/7zz"
cp "$root/third_party/chdman/linux/chdman" "$work/chdman"
chmod u+x "$work/7zz" "$work/chdman"

mkdir "$work/source" "$work/extracted"
printf 'Dogu Linux smoke test\n' > "$work/source/archivo prueba.txt"
(
  cd "$work/source"
  "$work/7zz" a "$work/test.7z" 'archivo prueba.txt' >/dev/null
)
"$work/7zz" x "$work/test.7z" "-o$work/extracted" -y >/dev/null
cmp "$work/source/archivo prueba.txt" "$work/extracted/archivo prueba.txt"
"$work/7zz" i | grep -q 'Rar5'

dd if=/dev/zero of="$work/test.iso" bs=1048576 count=2 status=none
"$work/chdman" createdvd -i "$work/test.iso" -o "$work/test.chd" >/dev/null 2>&1
"$work/chdman" extractdvd -i "$work/test.chd" -o "$work/restored.iso" >/dev/null 2>&1
cmp "$work/test.iso" "$work/restored.iso"
echo 'Linux sidecar smoke tests passed'
