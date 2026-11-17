#!/bin/sh
# Installs 3D Viewer for the current user (no root needed).
set -e
HERE="$(cd "$(dirname "$0")" && pwd)"
BIN="$HOME/.local/bin"
APPS="$HOME/.local/share/applications"
ICONS="$HOME/.local/share/icons/hicolor/256x256/apps"
mkdir -p "$BIN" "$APPS" "$ICONS"
install -m 755 "$HERE/viewer3d" "$BIN/viewer3d"
install -m 644 "$HERE/viewer3d.png" "$ICONS/viewer3d.png"
# Use the full path so the menu entry works even if ~/.local/bin is not in PATH.
sed "s|^Exec=viewer3d|Exec=$BIN/viewer3d|" "$HERE/viewer3d.desktop" > "$APPS/viewer3d.desktop"
command -v update-desktop-database >/dev/null 2>&1 && update-desktop-database "$APPS" >/dev/null 2>&1 || true
command -v gtk-update-icon-cache >/dev/null 2>&1 && gtk-update-icon-cache -q "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true
echo "3D Viewer installed."
echo "Start it from the application menu, or run: $BIN/viewer3d"
