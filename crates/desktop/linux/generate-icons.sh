#!/bin/sh
# Regenerates the hicolor icon set in icons/ from the brand logo: one PNG per size, named massorca.png,
# the name the desktop entry gives. Needs rsvg-convert (package librsvg2-bin). Run it only when the logo
# changes: sh crates/desktop/linux/generate-icons.sh
set -eu
here=$(cd "$(dirname "$0")" && pwd)
logo="$here/../../ui/assets/brand/massorca-logo.svg"
for size in 48 64 128 256; do
    dir="$here/icons/hicolor/${size}x${size}/apps"
    mkdir -p "$dir"
    rsvg-convert --width "$size" --height "$size" --format png --output "$dir/massorca.png" "$logo"
done
