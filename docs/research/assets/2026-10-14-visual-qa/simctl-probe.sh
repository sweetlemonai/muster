#!/bin/sh
# mu-130 probe: an iOS simulator driven and captured headless, in a device set of its own.
# Usage: simctl-probe.sh <out-dir> [runtime-id] [device-type-id]
# `--set` keeps the probe's device out of the owner's device list, and nothing opens Simulator.app,
# so no window appears. Run outside Claude Code's Bash sandbox (CoreSimulator is unreachable inside).
set -eu
[ $# -ge 1 ] || { echo "usage: simctl-probe.sh <out-dir> [runtime-id] [device-type-id]" >&2; exit 2; }
out=$1
mkdir -p "$out/devices"
set_dir=$(cd "$out/devices" && pwd)
sim() { xcrun simctl --set "$set_dir" "$@"; }
# The pixel diff behind "differed in N% of pixels" (sck-probe diff: largest per-channel delta > 0).
here=$(cd "$(dirname "$0")" && pwd)
swiftc -O -o "$out/sck-probe" "$here/sck-probe.swift"
# The last available iOS runtime listed (or the one given), and the last iPhone device type that
# runtime lists (or the one given). "Last listed" isn't "newest": on iOS 26.5 it was an iPhone 11,
# so pass a device type (`xcrun simctl list devicetypes`) when the model matters.
pick=$(xcrun simctl list runtimes -j | python3 -I -c '
import json, sys
want = sys.argv[1]
ios = [r for r in json.load(sys.stdin)["runtimes"]
       if r["platform"] == "iOS" and r["isAvailable"] and (not want or r["identifier"] == want)]
if ios:
    phones = [d["identifier"] for d in ios[-1]["supportedDeviceTypes"] if d["productFamily"] == "iPhone"]
    print(ios[-1]["identifier"], phones[-1] if phones else "")' "${2:-}")
[ -z "${3:-}" ] || pick="${pick% *} $3"
runtime=${pick% *}
device_type=${pick#* }
[ -n "$runtime" ] && [ -n "$device_type" ] || { echo "no iOS runtime with an iPhone installed" >&2; exit 1; }

udid=$(sim create mu130-probe "$device_type" "$runtime")
trap 'sim shutdown "$udid" >/dev/null 2>&1 || true; sim delete "$udid" >/dev/null 2>&1 || true' EXIT
echo "== device $device_type on $runtime"
start=$(date +%s)
sim boot "$udid"
sim bootstatus "$udid" >/dev/null
echo "booted headless in $(( $(date +%s) - start )) s"

echo "== deterministic state: status bar, appearance"
sim status_bar "$udid" override --time 9:41 --batteryState charged --batteryLevel 100 --cellularBars 4 --wifiBars 3
sim ui "$udid" appearance dark
sim ui "$udid" appearance

echo "== screenshot twice, a second apart"
sim io "$udid" screenshot --mask=black "$out/shot-1.png" 2>/dev/null
sleep 1
sim io "$udid" screenshot --mask=black "$out/shot-2.png" 2>/dev/null
sips -g pixelWidth -g pixelHeight "$out/shot-1.png" | tail -2
"$out/sck-probe" diff "$out/shot-1.png" "$out/shot-2.png" |
  awk '$1 == "pixels" && $2 > 0 {printf "%s (%.1f%% of pixels differ)\n", $0, 100 * $4 / $2; next} {print}'

echo "== video while a deep link opens Settings, then idle"
# A non-interactive shell starts background jobs with SIGINT ignored, and simctl inherits that, so
# the recorder is started through a wrapper that restores SIGINT; otherwise it never stops.
python3 -I -c 'import os, signal, sys; signal.signal(signal.SIGINT, signal.SIG_DFL); os.execvp(sys.argv[1], sys.argv[1:])' \
  xcrun simctl --set "$set_dir" io "$udid" recordVideo --codec=h264 --force "$out/rec.mov" 2>"$out/rec.log" &
rec=$!
sleep 2
sim openurl "$udid" "App-prefs:"
sleep 3
kill -INT "$rec"
wait "$rec" || true
grep -m1 "Recording started" "$out/rec.log" || true
ffprobe -v error -count_frames -select_streams v:0 \
  -show_entries stream=codec_name,width,height,nb_read_frames:format=format_name,duration \
  -of default=noprint_wrappers=1 "$out/rec.mov"
sim io "$udid" screenshot --mask=black "$out/settings.png" 2>/dev/null
echo "screenshot after the deep link: $out/settings.png"
