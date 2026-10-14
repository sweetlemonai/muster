#!/bin/sh
# Rebuilds every bundle icon from the owner's art (task 0085). Needs Xcode 26 or newer (actool).
# The outputs are committed, so run it only after the
# art changes:
#   - Muster.icon (Icon Composer: the robot on its fill, light and dark, from the art in original/)
#     → Assets.car (macOS 26's icon, light, dark and tinted) and icon.icns (the flat fallback).
#   - original/png/muster-app-icon-dark-1024.png → the PNGs and icon.ico, through `tauri icon`.
# To check the result: `xcrun assetutil --info Assets.car` lists the light and dark
# (NSAppearanceNameDarkAqua) renditions; CFBundleIconName in ../Info.plist names "Muster".
set -eu
cd "$(dirname "$0")"
out=$(mktemp -d "${TMPDIR:-/tmp}/muster-icons.XXXXXX")
trap 'rm -rf "$out"' EXIT

xcrun actool Muster.icon --compile "$out" --platform macosx --minimum-deployment-target 10.13 \
  --app-icon Muster --output-partial-info-plist "$out/partial.plist" >/dev/null
cp "$out/Assets.car" Assets.car
cp "$out/Muster.icns" icon.icns

../../node_modules/.bin/tauri icon original/png/muster-app-icon-dark-1024.png -o "$out/tauri" >/dev/null
for f in 32x32.png 64x64.png 128x128.png 128x128@2x.png icon.png icon.ico Square30x30Logo.png \
  Square44x44Logo.png Square71x71Logo.png Square89x89Logo.png Square107x107Logo.png \
  Square142x142Logo.png Square150x150Logo.png Square284x284Logo.png Square310x310Logo.png \
  StoreLogo.png; do
  cp "$out/tauri/$f" "$f"
done
echo "icons: rebuilt Assets.car, icon.icns and the PNGs from original/ and Muster.icon"
