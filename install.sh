#!/usr/bin/env bash
set -e

# Detect target directory: default to ~/.local (no root required), or /usr/local if root or --system
if [ "$EUID" -eq 0 ] || [ "$1" = "--system" ]; then
    PREFIX="${PREFIX:-/usr/local}"
    BIN_DIR="$PREFIX/bin"
    APPS_DIR="$PREFIX/share/applications"
    ICONS_DIR="$PREFIX/share/icons/hicolor/scalable/apps"
else
    PREFIX="${PREFIX:-$HOME/.local}"
    BIN_DIR="$PREFIX/bin"
    APPS_DIR="$PREFIX/share/applications"
    ICONS_DIR="$PREFIX/share/icons/hicolor/scalable/apps"
fi

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "Installing SoundCloud to $PREFIX..."

mkdir -p "$BIN_DIR" "$APPS_DIR" "$ICONS_DIR"

# Install binary
if [ -f "$SCRIPT_DIR/soundcloud" ]; then
    install -m 755 "$SCRIPT_DIR/soundcloud" "$BIN_DIR/soundcloud"
    ln -sf "soundcloud" "$BIN_DIR/cloudpulse"
elif [ -f "$SCRIPT_DIR/target/release/cloudpulse" ]; then
    install -m 755 "$SCRIPT_DIR/target/release/cloudpulse" "$BIN_DIR/soundcloud"
    ln -sf "soundcloud" "$BIN_DIR/cloudpulse"
else
    echo "Error: soundcloud binary not found in $SCRIPT_DIR!" >&2
    exit 1
fi

# Install desktop files
if [ -f "$SCRIPT_DIR/org.cloudpulse.SoundCloud.desktop" ]; then
    install -m 644 "$SCRIPT_DIR/org.cloudpulse.SoundCloud.desktop" "$APPS_DIR/org.cloudpulse.SoundCloud.desktop"
    ln -sf "org.cloudpulse.SoundCloud.desktop" "$APPS_DIR/soundcloud.desktop"
    ln -sf "org.cloudpulse.SoundCloud.desktop" "$APPS_DIR/cloudpulse.desktop"
elif [ -f "$SCRIPT_DIR/soundcloud.desktop" ]; then
    install -m 644 "$SCRIPT_DIR/soundcloud.desktop" "$APPS_DIR/org.cloudpulse.SoundCloud.desktop"
    ln -sf "org.cloudpulse.SoundCloud.desktop" "$APPS_DIR/soundcloud.desktop"
    ln -sf "org.cloudpulse.SoundCloud.desktop" "$APPS_DIR/cloudpulse.desktop"
fi

# Install icons
ICON_SRC=""
if [ -f "$SCRIPT_DIR/soundcloud.svg" ]; then
    ICON_SRC="$SCRIPT_DIR/soundcloud.svg"
elif [ -f "$SCRIPT_DIR/assets/soundcloud_app_icon.svg" ]; then
    ICON_SRC="$SCRIPT_DIR/assets/soundcloud_app_icon.svg"
elif [ -f "$SCRIPT_DIR/org.cloudpulse.SoundCloud.svg" ]; then
    ICON_SRC="$SCRIPT_DIR/org.cloudpulse.SoundCloud.svg"
fi

if [ -n "$ICON_SRC" ]; then
    install -m 644 "$ICON_SRC" "$ICONS_DIR/org.cloudpulse.SoundCloud.svg"
    ln -sf "org.cloudpulse.SoundCloud.svg" "$ICONS_DIR/soundcloud.svg"
    ln -sf "org.cloudpulse.SoundCloud.svg" "$ICONS_DIR/cloudpulse.svg"
fi

# Update desktop and icon databases
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$APPS_DIR" 2>/dev/null || true
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -q -t "$(dirname "$ICONS_DIR")" 2>/dev/null || true
fi

echo "Installation complete!"
echo "Run 'soundcloud' in terminal or launch SoundCloud from your app menu."
