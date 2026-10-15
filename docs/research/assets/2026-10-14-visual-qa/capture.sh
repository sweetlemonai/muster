#!/bin/sh
# mu-130 probe: the window-capture results in §4 and §6 of the report, rerunnable.
# Usage: capture.sh <window-id> <out-dir>
# Pick the id from `sck-probe list` (an app window that's already open). Nothing is clicked, typed
# or raised. Run outside Claude Code's Bash sandbox; the terminal (or whatever launched this) needs
# Screen Recording. To repeat the off-Space rows, pass a window that is on another Space.
set -eu
[ $# -eq 2 ] || { echo "usage: capture.sh <window-id> <out-dir>" >&2; exit 2; }
id=$1
out=$2
here=$(cd "$(dirname "$0")" && pwd)
mkdir -p "$out"
probe="$out/sck-probe"
swiftc -O -o "$probe" "$here/sck-probe.swift"

frames() { # <video>: frame count, duration in seconds, average fps, codec, size
  [ -s "$1" ] || { echo "no video at $1"; return 0; }
  ffprobe -v error -select_streams v:0 -count_frames \
    -show_entries stream=codec_name,width,height,nb_read_frames:format=duration \
    -of default=noprint_wrappers=1 "$1" |
    awk -F= '{v[$1]=$2} END {d = v["duration"] + 0
      if (d <= 0) {print "unreadable video (no duration)"; exit}
      printf "%s %sx%s frames %s duration %.2fs avg %.1f fps\n",
        v["codec_name"], v["width"], v["height"], v["nb_read_frames"], d, v["nb_read_frames"] / d}' ||
    echo "ffprobe couldn't read $1"
}

echo "== stills: screencapture -l, twice, a second apart"
screencapture -x -o -l "$id" "$out/sc-1.png"
sleep 1
screencapture -x -o -l "$id" "$out/sc-2.png"
sips -g pixelWidth -g pixelHeight -g profile "$out/sc-1.png" | tail -3
"$probe" diff "$out/sc-1.png" "$out/sc-2.png"

echo "== stills: ScreenCaptureKit, twice, a second apart"
"$probe" shot "$id" "$out/sck-1.png"
sleep 1
"$probe" shot "$id" "$out/sck-2.png"
sips -g profile "$out/sck-1.png" | tail -1
"$probe" diff "$out/sck-1.png" "$out/sck-2.png"

echo "== stills: screencapture vs ScreenCaptureKit"
"$probe" diff "$out/sc-1.png" "$out/sck-1.png"

echo "== video: ScreenCaptureKit, 3 s, 30 fps cap"
"$probe" rec "$id" "$out/sck.mp4" 3 || echo "ScreenCaptureKit recording failed"
frames "$out/sck.mp4"

echo "== video: screencapture -V 2 -l"
screencapture -x -V 2 -l "$id" "$out/sc.mov" || echo "screencapture recording failed"
frames "$out/sc.mov"
# A black recording (the off-Space case) has a mean luma near 16 on every frame.
luma() { # <label> <video>
  [ -s "$2" ] || return 0
  ffmpeg -v error -i "$2" -fps_mode passthrough \
    -vf "signalstats,metadata=print:key=lavfi.signalstats.YAVG:file=-" -f null - |
    awk -F= -v label="$1" '/YAVG/ {s += $2; n++} END {if (n) printf "%s video mean luma %.1f over %d frames\n", label, s / n, n}'
}
luma screencapture "$out/sc.mov"
luma ScreenCaptureKit "$out/sck.mp4"
