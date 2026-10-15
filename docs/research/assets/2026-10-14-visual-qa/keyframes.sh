#!/bin/sh
# mu-130 probe: turns a recording into the few frames a vision model judges, and one contact sheet.
# Usage: keyframes.sh <video> <out-dir> [scene-threshold, default 0.1]
# Scene-change frames (the select filter's scene score) plus the first and last frame, then a tile.
set -eu
[ $# -ge 2 ] || { echo "usage: keyframes.sh <video> <out-dir> [scene-threshold]" >&2; exit 2; }
video=$1
out=$2
threshold=${3:-0.1}
mkdir -p "$out"
ffmpeg -v error -y -i "$video" -frames:v 1 "$out/first.png"
ffmpeg -v error -y -sseof -0.1 -i "$video" -update 1 -frames:v 1 "$out/last.png"
ffmpeg -v error -y -i "$video" -vf "select='gt(scene,$threshold)'" -fps_mode vfr "$out/scene-%03d.png"
echo "scene frames over $threshold: $(find "$out" -name 'scene-*.png' | wc -l | tr -d ' ')"
ffmpeg -v error -y -i "$video" -vf "select='eq(n,0)+gt(scene,$threshold)',scale=320:-2,tile=4x2:padding=4" \
  -frames:v 1 -fps_mode vfr "$out/contact-sheet.png"
sips -g pixelWidth -g pixelHeight "$out/contact-sheet.png" | tail -2
