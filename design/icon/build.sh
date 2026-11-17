#!/usr/bin/env bash
# Regenerates every icon in app/src-tauri/icons/ from the sources in this folder.
#
#   design/icon/build.sh            # everything
#   design/icon/build.sh previews   # only design/icon/preview/*.png
#
# Needs: pnpm (Tauri CLI from app/), and on macOS Xcode 26 or later for actool
# (Assets.car) and Icon Composer's ictool (previews). Off macOS the Assets.car
# and preview steps are skipped and the committed files are kept.
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo="$(cd "$here/../.." && pwd)"
icons="$repo/app/src-tauri/icons"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

previews() {
  local ictool
  ictool="$(xcode-select -p)/../Applications/Icon Composer.app/Contents/Executables/ictool"
  if [ ! -x "$ictool" ]; then
    echo "ictool not found; skipping previews" >&2
    return
  fi
  mkdir -p "$here/preview"
  for rendition in Default Dark TintedLight TintedDark ClearLight ClearDark; do
    "$ictool" "$here/AppIcon.icon" --export-image \
      --output-file "$here/preview/$rendition.png" \
      --platform macOS --rendition "$rendition" \
      --width 256 --height 256 --scale 2 </dev/null >/dev/null
  done
  echo "previews: $here/preview"
}

if [ "${1:-}" = "previews" ]; then
  previews
  exit 0
fi

# 1. Liquid Glass icon: AppIcon.icon -> Assets.car (asset name "AppIcon").
#    actool wants absolute paths and an open stdin; --app-icon must equal the
#    .icon file stem. The .icns and plist it also writes are not used.
if [ "$(uname)" = "Darwin" ]; then
  mkdir "$work/car"
  xcrun actool "$here/AppIcon.icon" --compile "$work/car" \
    --app-icon AppIcon --include-all-app-icons \
    --platform macosx --target-device mac --minimum-deployment-target 11.0 \
    --enable-on-demand-resources NO --development-region en \
    --output-partial-info-plist "$work/car/partial.plist" \
    --output-format human-readable-text --warnings --errors </dev/null
  cp "$work/car/Assets.car" "$icons/Assets.car"
else
  echo "not macOS; keeping the committed Assets.car" >&2
fi

# 2. Classic set. Windows and Linux use the near full-bleed tile; the .icns
#    (macOS before 26) uses the tile on Apple's 824/1024 grid with its margin.
cd "$repo/app"
pnpm exec tauri icon "$here/icon-full.svg" --output "$work/full"
pnpm exec tauri icon "$here/icon-macos.svg" --output "$work/macos"
rm -rf "$work/full/ios" "$work/full/android"
find "$icons" -maxdepth 1 -type f \( -name '*.png' -o -name '*.ico' -o -name '*.icns' \) -delete
cp "$work"/full/*.png "$work/full/icon.ico" "$icons/"
cp "$work/macos/icon.icns" "$icons/icon.icns"

previews
echo "icons: $icons"
