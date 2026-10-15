# Probes behind the [local] results in 2026-10-14-visual-qa-drivers.md

Every **[local]** result in the report comes from one of these, run on macOS 26.5 (25F71), Xcode
26.6, Playwright 1.64.1 and ffmpeg from Homebrew. None of them clicks, types or raises a window on
the owner's screen (docs/GATES.md G4): `capture.sh` reads a window that's already open, the others
render off-screen, headless or in a simulator with no Simulator.app window. All of them but
`snapshot-probe.swift` and `sck-probe diff` run outside Claude Code's Bash sandbox, which blocks the
WindowServer, CoreSimulator and WebKit's launch.

| Probe | What it checks | Run | Result on 2026-10-14 |
|---|---|---|---|
| `sck-probe.swift` | the ScreenCaptureKit CLI: `list`, `shot`, `rec`, `diff` | `swiftc -O -o "$TMPDIR/sck-probe" sck-probe.swift` | builds with no warnings; `rec` exits 1 when the recording fails or doesn't finish |
| `capture.sh` | stills and video of one window by `screencapture` and by ScreenCaptureKit, their pixel diffs, frame rates and luma | `./capture.sh <window-id> "$TMPDIR/cap"` (id from `sck-probe list`) | see below |
| `snapshot-probe.swift` | a SwiftUI view rendered off-screen (`NSHostingView` + `cacheDisplay`, and `ImageRenderer`), light and dark, and the process's window count | `swiftc -O -parse-as-library -o "$TMPDIR/snapshot-probe" snapshot-probe.swift`, then run it twice, `"$TMPDIR/snapshot-probe" <dir>/1` and `<dir>/2`, and `cmp` each PNG pair | 0 windows; the three PNGs byte-identical between runs. Rerun at 20:50 inside the sandbox: `NSHostingView` PNGs 360×168 (scale 1 there), `ImageRenderer` 720×336 |
| `playwright-probe.mjs` | the walkthrough overlay (`addInitScript`) across a navigation, verdicts through `exposeBinding`, masked vs frozen-clock screenshots, trace and video files | setup in the file's header, then `cd "$TMPDIR/pw" && node <path>/playwright-probe.mjs <out-dir>` | see below |
| `simctl-probe.sh` | an iOS simulator in its own device set: headless boot, status bar, dark mode, screenshots and their pixel diff, a deep link, video | `./simctl-probe.sh "$TMPDIR/sim" [runtime-id] [device-type-id]` | see below |
| `keyframes.sh` | a recording cut to scene-change frames and a contact sheet | `./keyframes.sh <video> <out-dir> [0.1]` into an empty `<out-dir>` (it counts every `scene-*.png` there) | 174-frame simulator clip → 1 scene frame + first + last |

`sck-probe diff` draws both PNGs into sRGB and counts pixels whose largest per-channel difference is
above 0, and above 24. It reads files only, so it also runs inside the sandbox.

Where each figure comes from. The probes print most of them; this round (review 2, R12) added the
lines that print the rest. Those additions are syntax-checked (`shellcheck`, `node --check`) but
weren't rerun this round, which ran no simulator, Playwright or screen capture:

| Figure | Printed by |
|---|---|
| 0 of 8,056,320 px; 1,810 px at most 3/255 | `capture.sh` (its `sck-probe diff` lines: `differing`, `maxdelta`) |
| 18–22 and 28–34 fps, frame counts | `capture.sh` (`frames`) |
| 23% of simulator pixels differ | `simctl-probe.sh` (now `sck-probe diff` with the percentage; how the earlier run counted it isn't recorded) |
| 828×1792, 174 frames in 4.76 s | `simctl-probe.sh` (`sips`, `ffprobe`) |
| 38 px at the mask's edge | `sck-probe diff masked-1.png masked-2.png` on the probe's output (the probe prints only whether the hashes match; how the earlier run counted it isn't recorded) |
| WebM, VP9, 800×500, 25 fps | `playwright-probe.mjs` (now its last line, from `ffprobe`) |
| identical snapshots on a second run | two `snapshot-probe` runs and `cmp`, as in the table |

## Results

**`capture.sh` on the Muster window** (on screen, 2× display, 3840×2098 px):

- `screencapture -l` twice: 0 of 8,056,320 pixels differ. ScreenCaptureKit twice: 0 differ.
  `screencapture` vs ScreenCaptureKit: 0 differ. Both PNGs carry the display's ICC profile
  ("LG HDR 4K").
- ScreenCaptureKit video: H.264, 3840×2098, 55 frames in 3.06 s (18 fps at a 30 fps cap; frames
  arrive only when the window changes). `screencapture -V 2 -l`: H.264, 3976×2234 (the shadow
  margin), 68 frames in 2.01 s (34 fps).

**`capture.sh` on a Finder window listed `on-screen false`** (1×, 1481×954; whether it was
minimised or on another Space isn't known, and the probe doesn't move windows to find out):
stills by both routes worked and matched (0 differ); two ScreenCaptureKit stills differed in 1,810
pixels by at most 3/255. ScreenCaptureKit recording failed: `RPRecordingErrorDomain -5822`,
"Failed due to failure to process first sample buffer". The first run of this research (session
770b5534, 19:45) recorded real content from a window on another Space; this rerun didn't reproduce
it with an off-screen window, so the report no longer relies on it.

**`playwright-probe.mjs`**, headless WebKit:

- The overlay injected with `context.addInitScript` was back after a navigation, and both its
  buttons reached the runner: `[{"url":"a.html","verdict":"pass"…},{"url":"b.html","verdict":"fail"…}]`.
- A `mask` over a ticking clock: identical on one run, 38 pixels different at the mask's right edge
  on another (the box follows the text's width). The same page with `page.clock.setFixedTime`:
  identical. Unmasked: different, as expected.
- `trace.zip`: 256–358 KB for the run. Video: WebM, VP9, 800×500, 25 fps.
- `_enableRecorder` is gone in 1.64.1; `_startRecording`, `_stopRecording` and `_showRecorder`
  exist (private API).

**`simctl-probe.sh`**, iOS 26.5 runtime, iPhone 11 device type:

- Booted headless in 38 s (the device type was the runtime's last-listed iPhone, an iPhone 11; the
  script now takes a device type as its third argument); Simulator.app never opened and no permission prompt appeared.
- Status bar override (9:41, full battery) and dark appearance took effect in the screenshot.
- Two home-screen screenshots a second apart differed in 23% of pixels (springboard still settling
  after first boot); a capture needs a settle wait or a known app state.
- `openurl "App-prefs:"` opened Settings. `recordVideo --codec=h264`: QuickTime, 828×1792, 174
  frames in 4.76 s.
- Gotcha: a background job in a non-interactive shell ignores SIGINT, and simctl inherits that, so
  `kill -INT` never stops the recording. The probe starts it through a wrapper that restores SIGINT.
