#!/bin/sh
set -eu

root="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
bindir="${ATELIER_BINDIR:-$HOME/.local/bin}"
binary="${ATELIER_BINARY:-}"

if [ -z "$binary" ]; then
    if [ -x "$root/target/release/atelier" ]; then
        binary="$root/target/release/atelier"
    elif [ -x "$root/target/debug/atelier" ]; then
        binary="$root/target/debug/atelier"
    else
        echo "no atelier binary found; build one with cargo build or cargo build --release" >&2
        exit 1
    fi
fi

mkdir -p "$bindir"
install -Dm755 "$binary" "$bindir/atelier"

desktop_dir="$data_home/applications"
mkdir -p "$desktop_dir"
sed "s|^Exec=atelier %F$|Exec=$bindir/atelier %F|" \
    "$root/assets/atelier.desktop" >"$desktop_dir/atelier.desktop"

icon_root="$data_home/icons/hicolor"
for size in 16 24 32 48 64 128 256 512; do
    install -Dm644 \
        "$root/assets/icons/hicolor/${size}x${size}/apps/atelier.png" \
        "$icon_root/${size}x${size}/apps/atelier.png"
done
install -Dm644 \
    "$root/assets/icons/hicolor/scalable/apps/atelier.svg" \
    "$icon_root/scalable/apps/atelier.svg"

if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$desktop_dir"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$icon_root" >/dev/null 2>&1 || true
fi
if command -v xdg-desktop-menu >/dev/null 2>&1; then
    xdg-desktop-menu forceupdate >/dev/null 2>&1 || true
fi

echo "installed $bindir/atelier"
echo "installed $desktop_dir/atelier.desktop"
echo "installed icons in $icon_root"
