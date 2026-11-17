#!/bin/sh
# Makes a .tar.gz that Linux users can download: binary, icon, menu entry and install script.
# usage: packaging/linux/make-package.sh <path-to-viewer3d-binary> <output-name-without-extension>
set -e
BIN="$1"; NAME="$2"
[ -f "$BIN" ] && [ -n "$NAME" ] || { echo "usage: $0 <binary> <name>"; exit 1; }
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/../.." && pwd)"
OUT="$ROOT/dist"
rm -rf "$OUT/$NAME" && mkdir -p "$OUT/$NAME"
install -m 755 "$BIN" "$OUT/$NAME/viewer3d"
install -m 644 "$ROOT/assets/icon.png" "$OUT/$NAME/viewer3d.png"
install -m 644 "$HERE/viewer3d.desktop" "$HERE/README.txt" "$OUT/$NAME/"
install -m 755 "$HERE/install.sh" "$HERE/uninstall.sh" "$OUT/$NAME/"
tar -C "$OUT" -czf "$OUT/$NAME.tar.gz" "$NAME"
echo "$OUT/$NAME.tar.gz"
