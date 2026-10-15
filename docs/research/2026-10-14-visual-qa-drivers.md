# Visual QA on macOS 26: driving each app stack headless, capturing it, walking the owner through it, and judging it against a mockup (2026-10-14)

Task mu-130, for mu-133 (web walkthrough) and mu-141 (iOS, Mac) to build on. Researched by Pouya on
2026-10-14 and revised the same evening after review rounds 1 (Pouya_2) and 2 (Pouya_3), on macOS
26.5 (25F71) with Xcode 26.6 and Playwright 1.64.1. Every claim here comes from a cited source or
from a local test, and local tests are marked **[local]**. Each [local] result has its probe and exact commands in
[`assets/2026-10-14-visual-qa/`](assets/2026-10-14-visual-qa/README.md), rerun on 2026-10-14.
Recommendations are mine and are labelled.

The rule they follow is `docs/GATES.md` G4: everything an agent runs is headless, and it never
clicks, types or raises a window on the owner's screen. A visible app or browser opens only for a
walkthrough the owner starts. Native-only checks are screenshots of a window that's already open.

**Answer:** Agents check every stack without touching the owner's screen, and each stack gets a
headless route first:
- **Tauri and Electron:** headless Playwright WebKit (Chromium for Electron) on the frontend build,
  with the native side mocked: `mockIPC` for Tauri, a stub of the preload API for Electron. It
  covers every state the frontend can reach, with screenshots, video and traces [2][8][9].
- **iOS:** a simulator booted by `simctl` in a device set of its own, with no Simulator.app window.
  Status bar, appearance and deep links are set by command, and `simctl io` takes screenshots and
  video **[local]**. XCUITest reaches the deeper states [S7][S9].
- **Mac (SwiftPM and Xcode):** snapshot tests first. Views render off-screen to PNG with no window
  and no permission **[local]** [M1][M3]. A live app window is captured only when it's already
  open, by a small ScreenCaptureKit CLI that reads it and sends no input **[local]**. That window
  belongs to an instance started on fixture data (GATES.md Fixtures), never the owner's own.

The walkthrough the owner starts is the one place a window opens. For web stacks, Playwright
injects the step overlay with `addInitScript` and gets the verdicts back through `exposeBinding`
**[local]** [W1][W2]. Native apps get Muster's steps panel beside the app, as the proposal says,
and AX scripting drives a native app only inside a walkthrough the owner started.

A vision model should **not** be the detector. On rendered web UIs the best of 13 frontier models
found 40.7% of real changes, and under 23% of the hard ones [24]. So the measurable checks run
first: pixel diff against an approved baseline, WCAG contrast, bounds and clipping. The model then
explains or triages the regions those checks flagged.

**Confidence:** medium-high for the drivers and the capture path. They rest on vendor docs, Apple
DTS answers and a local test of each capture route. It's medium for TCC persistence, because
Apple doesn't document it and the sources are DTS forum answers plus community reports. It's
medium for the vision-model numbers: they come from recent papers on web UIs, and none of them
covers macOS app windows.

## Recommendation per stack (mine)

"Agent" is what Kia and the other agents run unattended, headless under G4. "Walkthrough" is what
runs only when the owner starts one, in a window the owner sees.

| Stack | Agent: drive and reach states | Agent: screenshots and video | Walkthrough (owner-started) | TCC grants | Confidence |
|---|---|---|---|---|---|
| **Tauri 2** (Muster, Drover, braindump, germ) | Headless Playwright WebKit on the Vite build, `@tauri-apps/api/mocks` `mockIPC` for every state the frontend can reach, fixture data, clock frozen with `page.clock` [8] **[local]** | `page.screenshot` with `mask` and `animations: 'disabled'`; `recordVideo` (WebM, VP9, 25 fps cap) only for motion and flows; traces for evidence [6][W4][W5] **[local]** | Headed Playwright with the overlay injected by `addInitScript`, verdicts back through `exposeBinding` **[local]** [W1][W2]. The real app (WebdriverIO's embedded plugin [3][4]) only if the walkthrough needs native menus or IPC | none for the headless route | medium-high |
| **Electron** (choopan) | Headless Chromium on the renderer build with a stub of the preload API, as for Tauri. `_electron.launch` opens a real window (Electron has no headless mode [9][W7]), so it isn't an agent route | as for Tauri | `_electron.launch` from 1.64.1 with the overlay injected into its window; first-window wait and retry for #43247 [9][13] | none for the headless route | medium |
| **iOS** (planned apps) | `simctl --set <own dir>` create and boot, no Simulator.app; `status_bar override`, `ui appearance`, `openurl` deep links, `launch` arguments; XCUITest through `xcodebuild test` for deeper states **[local]** [S1][S3][S9] | `simctl io screenshot --mask=black`; `recordVideo --codec=h264` (stopped with SIGINT) **[local]** [S1]; XCUITest attachments via `xcresulttool export attachments` [S6] | Muster's steps panel beside a visible simulator; the owner annotates a captured screenshot (the proposal); auto mode later through XCUITest | none seen on this Mac **[local]**; unconfirmed by a source (Gaps) | medium |
| **Mac** (hood, cropshot, negar, porch, upstay, veneer, onetwoseven) | **Snapshot tests first:** views rendered off-screen (`NSHostingView` + `cacheDisplay`, as swift-snapshot-testing does) under `swift test`, light and dark, fixture data, no window **[local]** [M1][M2]. No agent driver for the live app | snapshot PNGs; for a window that's already open, the capture helper's `shot` (no input) **[local]**. That window belongs to an instance started on synthetic fixture data, with `HOME` and the app's config in `$TMPDIR` (GATES.md Fixtures), and is matched by that instance's PID. Never the owner's working instance and never another app's window, since captures are kept and sent to the vision model | Muster's steps panel beside the app window; AX scripting (`AXPress`, set value) only to automate a walkthrough the owner started | none for snapshots. Screen Recording for window capture, held only by the signed capture helper (§4) | medium-high for snapshots, medium for capture |
| **Capture helper** (Mac windows) | — | `SCScreenshotManager.captureImage` with `SCContentFilter(desktopIndependentWindow:)`, `ignoreShadowsSingleWindow`; `SCStream` + `SCRecordingOutput` to H.264 `.mp4` **[local]** [52] | used for evidence in native walkthroughs | Screen Recording, granted to this helper alone (§4) | high for the capture calls (tested, rerunnable); the helper's packaging isn't built yet |

On mu-133's and mu-141's design (also mine): the web adapters need no new tool, only Playwright,
which isn't a Muster dependency yet (the owner's yes and `/deps-audit vet` first). The Mac adapter
is snapshot tests in each app's test target, plus one capture helper with `shot` and `rec` for
windows that are already open; an `ax` subcommand belongs to the walkthrough runner, never to an
agent's check. The helper is a small background `.app` bundle signed with a stable identity and
launched through LaunchServices (or as a launchd agent), not exec'd from the terminal, so it, not
the terminal, is the process TCC checks [22], and its grant survives rebuilds (§4). It alone holds Screen
Recording, and it accepts only a window id whose owner PID is the fixture instance the runner
launched. It runs outside Claude Code's Bash sandbox, which blocks the WindowServer **[local]**.

## Findings

### 1. Tauri 2

- **Stock `tauri-driver` still doesn't support macOS.** Tauri's docs say "only Windows and Linux are
  supported on desktop, as macOS has no WKWebView driver tool available" [1].
- **There are in-app WebDriver servers for macOS, and they work today:**
  - CrabNebula's fork covers macOS but "a paid API key is required for macOS" [1], so it's out.
  - WebdriverIO's Tauri service supports macOS through an embedded WebDriver server,
    `tauri-plugin-wdio-webdriver`, with "No external driver needed (no CrabNebula subscription)"
    [3]. The plugin is added as `tauri-plugin-wdio-webdriver = "1"` under debug-only dependencies.
    It needs Tauri 2.0+ and the capability `wdio-webdriver:default` [4]. `@wdio/tauri-service` is
    at 1.5.0 on npm **[local]**.
  - Choochmeque's MIT `tauri-plugin-webdriver` drives WKWebView through its native APIs, works
    with Selenium and WebdriverIO, and supports screenshots. It listens on 127.0.0.1:4445 and
    "should never be included in production builds". Its last commit was 2026-10-04, and it has
    about 39 stars [5].
  - Daniel Raffel's `tauri-wd` is macOS-only. It covers "a surprisingly large subset" of WebDriver
    and has an MCP server for agents [7].
- **Playwright WebKit with `mockIPC`** tests a different engine, not the app:
  - Playwright's WebKit is built from WebKit main with Playwright's own patches. It isn't Safari or
    the system WKWebView [2].
  - `mockIPC` runs with "no real webview or Rust backend". `emitTo` and `emit_filter` aren't
    supported, and `mockWindows` only fakes window labels [8].
  - What it misses against the release app: real IPC and Rust commands, native menus and tray,
    file dialogs, window management, custom protocols, packaging and permissions [8][12].
- **Playwright `recordVideo` on WebKit:**
  - 1.64 (2026-10-07) added an `fps` option. WebKit captures at most 25 fps, and 25 is the default
    [6].
  - The size defaults to the viewport scaled to fit 800×800, or 800×450 with no viewport set [9].
  - Videos are written only when the browser context closes [9].
  - 1.64 says videos are now "encoded with VP9 instead of VP8" [6]. The shared recorder hardcodes
    `libvpx-vp9` and a `.webm` path for every browser [W8]. A WebKit run here gave WebM, VP9,
    800×500 at 25 fps **[local]**.
- **Under G4 (mine):** only the Playwright route is an agent route. Every WebDriver plugin above
  drives the real app, which opens its window on the owner's screen, so they belong to walkthroughs
  the owner starts, and only when one needs native menus, dialogs or real IPC.

### 2. Electron

- **Status.** Playwright's Electron support is "experimental" in Playwright's docs, and Electron v21+
  is supported [9]. Electron's testing guide lists it as one option beside WebdriverIO, Selenium and
  a custom driver, and ranks none of them [10].
- **No headless mode.** Electron has none, and `electron.launch` has no option to hide the window.
  The app must create its windows with `show: false` itself, and such a window still reports
  `visibilityState` "visible" [W7]. `webContents.capturePage()` on a hidden window makes the page
  visible unless `stayHidden` is set [W9]. So under G4 the agent route for choopan is its renderer
  in headless Chromium with the preload API stubbed, and `_electron.launch` is for walkthroughs.
- **Known breakages in 2026:**
  - January: Electron 30+ rejected `--remote-debugging-port=0`. A fix was merged the same day
    [11], then reverted on 2026-03-16 after regressions in VS Code and Electron 39 [12b].
  - Playwright 1.64.0 intermittently loses a packaged app's first window, reported on Electron
    44.5.1, including hidden windows with no renderer yet [13]. The issue is open and targeted at
    1.65, with an upstream Chromium fix linked on 2026-10-14. 1.64.1 (current on npm on 2026-10-14
    **[local]**) isn't said to fix it. Mitigation: wait for the first window with a timeout and
    relaunch once, or stay on 1.63 for Electron runs.
- **Fuse.** If launch times out, the `EnableNodeCliInspectArguments` fuse must not be off. That
  fuse gates `--inspect` [9][19]. A packaged choopan build with the fuse flipped won't launch
  under Playwright, so test builds keep it on.
- **Video and screenshots.** `recordVideo` works in `electron.launch` with `dir`, `size` and `fps`
  (25 by default), and files are saved after `browserContext.close()` [9]. It records the page.
  No source says it captures native window chrome or menus. That it doesn't is my inference from
  the CDP page model, so window chrome goes through the SCK CLI.
- **Alternatives.** Spectron was deprecated in February 2022 [20]. WebdriverIO's Electron service
  is now first-party as `@wdio/electron-service` [21] (10.4.0 on npm **[local]**). It's the
  fallback if Playwright's Electron mode breaks.

### 3. SwiftPM and Xcode apps

- **Snapshot tests first** (the proposal's order, and G4's: no window, no input, no permission).
  - swift-snapshot-testing 1.19.6 (2026-09-21) has a macOS `.image` strategy for `NSView` and
    `NSViewController`, and Swift Testing support since 1.19.0 [M1][M2]. It has no macOS strategy
    for a SwiftUI view, so the view goes in an `NSHostingView` [M2].
  - Its NSView strategy draws with `bitmapImageRepForCachingDisplay` and `cacheDisplay`, with no
    window [M2]. A probe doing the same with a SwiftUI view rendered PNGs (720×336 from
    `ImageRenderer`; 360×168 from `NSHostingView` when rerun inside the sandbox, where the scale was
    1), created **0 windows**, and gave byte-identical files on a second run **[local]**.
  - What it misses **[local]**: in dark appearance the view has no window background, so the PNG is
    white text on transparent; give the snapshot an explicit background. A material renders flat,
    not blurred. SwiftUI's `ImageRenderer` drew an AppKit-backed `Toggle` as a placeholder, which
    Apple documents for "most types of UIKit and AppKit views" [M3]. Liquid Glass needs a host app
    to snapshot on iOS 26 [M4]; on macOS it wasn't tested.
  - The scale follows the machine unless the strategy pins it, and window chrome isn't in the image
    [M5][51]. Keep baselines per machine class, as §6 says.
  - Window capture (§4) is the second step, for what a snapshot can't show: chrome, toolbar,
    materials, the real window. AX scripting comes last, for walkthroughs only.
- **XCUITest needs a project.**
  - UI tests need a host app and a UI-test target in a project. A Swift package can't supply one
    [14]. That source is a 2021 Swift Forums answer, and I found nothing newer (see Gaps).
  - Workaround: XcodeGen has a `bundle.ui-testing` target type that can wrap a SwiftPM app [15].
  - The macOS runner (`YourAppUITests-Runner.app`) is the process TCC checks, and it needs
    Accessibility [16].
  - A separate Automation Mode gate prompts for a password. It's lifted once, as an admin, with
    `automationmodetool enable-automationmode-without-authentication` [16][17]. That command
    "grants no privacy access" [16].
  - On this Mac, `automationmodetool` reports "Automation Mode is disabled. This device requires
    user authentication" **[local]**. That's one owner step, needed only if XCUITest is used.
- **The Accessibility API as the driver.**
  - A CLI run from a terminal has the terminal as its "responsible code", so the terminal is the
    app that gets the grant. Apple calls the algorithm undocumented and liable to change [22].
  - An ad-hoc-signed build gets a cdhash-based designated requirement. After a rebuild, System
    Settings still shows the grant on but `AXIsProcessTrusted` returns false [23]. That's a
    community report. A stable signing identity avoids it [25].
  - SwiftUI's `.accessibilityIdentifier` is the hook for finding elements. Its mapping to
    `AXIdentifier` isn't documented by Apple (see Gaps).
- **Other drivers.**
  - Appium mac2 is XCTest-based, needs Accessibility for Xcode Helper, and its 3.x line requires
    Appium 3 [26].
  - Peekaboo is MIT and very active (v4.9.3 on 2026-10-14). It needs Accessibility and Screen
    Recording [27]. A re-signed Peekaboo CLI is "a different TCC client" [28]. Its agent mode can
    send screenshots to a model provider, and its telemetry wasn't verified, so the privacy rule
    means auditing it before use.
  - AppleScript / System Events fails with -25211 without Accessibility and -1743 without
    Automation [29].

### 3b. iOS (added with the 19:43 scope)

- **Headless simulator.** `simctl boot` doesn't open Simulator.app [S3]. A probe created an iPhone
  device on the iOS 26.5 runtime in a device set of its own (`simctl --set <dir>`, so the owner's
  devices are untouched), booted it in 38 s, and no window or permission prompt appeared
  **[local]**.
- **Screenshots:** `simctl io <dev> screenshot [--type] [--display] [--mask ignored|alpha|black]`,
  PNG by default, at the device's pixel size (828×1792 here, on an iPhone 11 device type: the probe
  took the runtime's last-listed iPhone, and now takes a device type as an argument) [S1]
  **[local]**. Two home-screen
  shots a second after first boot differed in 23% of pixels **[local]**, so capture after a settle
  wait in a known app state.
- **Video:** `recordVideo [--codec h264|hevc] [--mask] [--force]` writes a QuickTime movie, hevc by
  default, prints "Recording started" once the first frame is in, and stops on SIGINT [S1]. It
  writes frames only when the screen changes [S2]; 174 frames in 4.76 s here **[local]**. A
  background job in a non-interactive shell ignores SIGINT, and simctl inherits that, so a script
  must restore SIGINT or the recording never stops **[local]**.
- **Reaching a state without input:** `status_bar override` (9:41, battery, bars) and
  `ui appearance dark` took effect, and `openurl "App-prefs:"` opened Settings **[local]**. Also
  `launch` with arguments and `SIMCTL_CHILD_` environment variables [S4], `privacy grant` and
  `push` [S11]. `ui appearance` was reported to hang on a macOS 26 CI runner; bound it with a
  timeout [S8].
- **XCUITest:** needs an app target and a UI-test target, so a SwiftPM-only app needs a runner app
  or a generated project [S7][15]. `xcodebuild test -destination 'platform=iOS Simulator,…'` boots
  the simulator itself [S9]. Attachments come out with `xcresulttool export attachments` (Xcode 16+)
  [S6]. One issue says simulator UI tests don't need macOS Automation Mode [S10]; unconfirmed.
- **Snapshots:** swift-snapshot-testing compares only on the exact simulator that recorded the
  reference [M2].

### 4. Window capture and TCC

- **Local results on macOS 26.5** (window on a 2× display, 3840×2098 px) **[local]**, from
  `capture.sh` and `sck-probe.swift` in the assets folder. Ranges cover the first run (session
  770b5534, 19:45) and the rerun (20:15):

| Route | Still | Video |
|---|---|---|
| `screencapture -x -o -l <id>` | ✅ window only, no shadow, Retina px. Works even when the window is on another Space | — |
| `screencapture -x -V 2 -l <id>` | — | ✅ while the window is on the active Space: window-only H.264 `.mov`, *with* shadow margin (4064×2322, then 3976×2234), 28–34 fps average, variable frame rate. ❌ All black once the window was on another Space, with or without `-o` (first run) |
| SCK CLI, `desktopIndependentWindow` | ✅ identical pixels to `screencapture -l` (0 differing of 8,056,320, both runs) | ✅ H.264 `.mp4`, 3840×2098, 18–22 fps average at a 30 fps cap. The first run recorded real content from a window on another Space; the rerun, on a window listed off-screen, failed with `RPRecordingErrorDomain -5822`. Not relied on |
| Any of these inside Claude Code's Bash sandbox | ❌ The binary aborts (`CGS_REQUIRE_INIT`, or a crash on the window list). It must run outside the sandbox | ❌ |

  The man page documents `-v`/`-V` only as "video recording of the screen" [30], so `-V -l`
  working at all is undocumented behaviour. I don't recommend building on it.
- **ScreenCaptureKit details.**
  - `SCRecordingOutput` writes a stream to a file with a chosen type and codec, and
    `startCapture` starts the recording too [18]. The macOS 26.5 SDK headers mark it macOS 15.0+,
    `SCScreenshotManager.captureImage(contentFilter:configuration:)` and
    `ignoreShadowsSingleWindow` macOS 14.0+ [52]. `desktopIndependentWindow` and
    `minimumFrameInterval` are used as in `sck-probe.swift`, which builds and runs **[local]**.
  - For Retina pixels, set the size to `contentRect × pointPixelScale`. There's no supported way
    to compute a shadow's frame [31].
  - A bare CLI needs `NSApplication.shared` (or another CoreGraphics init) before SCK, or it
    asserts `CGS_REQUIRE_INIT` **[local]**.
  - The scale follows the display the window is on: 2.0 on the 4K display, 1.0 on the ultrawide.
    So the same window gives different-sized captures depending on where it sits **[local]**.
  - Both routes tag the PNG with the *display's* ICC profile ("LG HDR 4K"), not sRGB **[local]**.
- **Finding the window.** `CGWindowListCopyWindowInfo` returns window ids, owner PID and bounds
  without Screen Recording. Only private values like `kCGWindowName` are withheld [32].
  `SCShareableContent` with `onScreenWindowsOnly: false` also lists off-Space windows
  **[local]**. Match on the owner PID of the app the driver launched, not on the title.
- **Which window may be captured (GATES.md Fixtures).** Only a window of an app instance the runner
  started on synthetic fixture data, with `HOME` and the app's config in `$TMPDIR`. Never the
  owner's working instance of the same app and never another app's window, because the captures
  are kept as evidence and sent to the vision model (§5, §8). The PID match above is what enforces
  it: the helper captures nothing whose owner PID isn't that instance's. The probes in the assets
  folder measure capture and take any window id (they ran on Muster's and a Finder window); they
  don't have that check, so they aren't the pattern for mu-141's helper.
- **TCC, spelled out:**

| Permission | Needed by | Granted to (the responsible process) | Survives updates? |
|---|---|---|---|
| **Screen & System Audio Recording** | Any capture: SCK CLI, `screencapture`, Peekaboo | The app that launched the CLI: Terminal/iTerm when run from them, VS Code when launched from it, the `.app` when the tool is embedded in a bundle and launched as an app [22][33]. On this Mac the Claude Code background process already passes `CGPreflightScreenCaptureAccess()` **[local]** | Yes, if the client has a stable signing identity [25]. An *ad-hoc* rebuild of a bundled helper is a new client; one signed with the same Developer ID or Apple Development identity isn't |
| **Accessibility** | The AX driver, System Events, Appium mac2 (Xcode Helper), the XCUITest runner | Same rule: the terminal for a CLI [22]. The `*-Runner.app` for XCUITest [16] | Same as above. A stale ad-hoc entry shows "on" but fails, and `tccutil reset Accessibility <id>` clears it [23] |
| **Automation (Apple Events)** | Only AppleScript / System Events | The calling app [29] | — |
| **Automation Mode** (not TCC) | XCUITest only | Machine-wide, `automationmodetool` run once as admin [16][17] | — |

  - `tccutil` can only *reset*, never grant [33].
  - A PPPC profile can't silently allow Screen Recording. It can only deny it or let standard
    users approve it [34]. So the first grant is a one-time owner click.
  - Sequoia added a recurring Screen Recording re-prompt, moved to monthly in beta [35], with
    "fewer dialogs" from 15.1 [36]. A managed `persistent-content-capture` entitlement is a
    possible, undocumented exemption: 9to5Mac calls it a potential solution, and Apple hasn't
    said how to get it [35][37].
  - In 26.1, plain executables vanished from the Screen Recording list. The original poster says
    the parent app is shown instead. Quinn (DTS) said "IMO this is a bug", which is his opinion,
    not Apple's position [33].
  - My recommendation: **a separately signed capture helper holds Screen Recording, and nothing
    else does.** Build the capture CLI as a background `.app` signed with a stable identity, launch
    it as an app so it is its own responsible process, and grant it once. It refuses any window id
    whose owner PID isn't the fixture instance the runner launched, so a grant to it covers one
    window of one test app, which is what G4 allows.
  - **Not the terminal.** Granting Screen Recording to the terminal that runs the agent sessions
    would let every process started from it capture the whole screen and any window: every agent
    session, repo scripts, package install hooks. That's far wider than G4's one already-open
    window, and against the baseline's narrowest-permission rule. If it's ever granted as a
    stop-gap while the helper is built, revoke it in System Settings › Privacy & Security once the
    helper works (or `tccutil reset ScreenCapture <terminal's bundle id>` [33]). The Claude Code
    background process on this Mac already holds the grant **[local]**; whether that was intended
    is the owner's call, and the same risk applies to it.
  - Accessibility waits until an AX-driven walkthrough is built, since no agent check uses it under
    G4. It follows the same rule: a signed helper, never the terminal.

### 5. Vision models judging a screen against its mockup

- **Detection rate.**
  - DiffSpot (2026-05) tested 13 frontier models on 1280×800 web UI pairs with one CSS property
    changed. The best, Gemini 3.1 Pro, found 40.7% of changes. Claude Opus 4.7 found 31.2% and
    GPT-5.4 30.5%, and every model was under 23% on hard changes [24].
  - The same models rarely invent changes: Opus 4.7 and GPT-5.4 reported a difference on 0.4% of
    identical pairs, while weaker models did so 12–24% of the time [24].
  - Some properties are nearly invisible to them: line height had a median recall of 4.0%, and
    rounded corners 13.3% [24]. Pixel magnitude doesn't predict what they catch (r = −0.08) [24].
- **Mockup vs app.** GUIPilot (ISSTA 2025) compared mobile screens with mockups:
  - Its deterministic widget-matching found inconsistencies at 94.5% precision and 99.6% recall
    [38]. The v1 paper's own abstract gives 99.8%/98.6%; I use the later abstract's figures.
  - Its VLM-only baseline scored 4.5–12.3% precision and 7.8–24.8% recall, with hallucinated
    inconsistencies and wrong widget ids [39]. That baseline model is unnamed and from 2025, so
    current models likely do better.
- **What helps, a little.** On VLM-SubtleBench (GPT-5, natural images):
  - A reasoning prompt raised accuracy from 71.3 to 73.1, highlighting changes made no
    difference, and **side-by-side concatenation hurt** (68.2). Humans scored 95.5 [40].
  - No paper shows overlay or diff-heatmap composites lifting recall on UI screenshots (see Gaps).
- **Resolution matters.** Claude downscales images whose long edge is over 1568 px on the standard
  tier, or 2576 px on the high-resolution tier, and a token budget often caps them sooner. A
  1920×1080 screenshot becomes 1456×819. For small targets the docs say to "crop the region of
  interest" or use a high-resolution-tier model [41]. OpenAI recommends `original` detail for small objects and
  warns about precise localisation [42]. A full 3840×2098 window capture loses detail before the
  model sees it.
- **What to measure instead:**
  - WCAG contrast is a formula, (L1+0.05)/(L2+0.05), with 4.5:1 for text and 3:1 for large text
    [43]. Compute it from pixels or styles.
  - Bounds, overlap and clipped text come from the DOM (Tauri, Electron) or AX frames (Swift).
  - Regressions come from a pixel diff.
  - Pixel diffs are blind to meaning and noisy, and change-captioning models still struggle on
    dense web UIs [44]. So the split I recommend is: detect with measurements, then describe with
    the model. Send it per-region crops at native resolution as two separate images, not one
    concatenated image, and ask for a checklist answer rather than "find the differences".

### 6. Pixel diff tolerances

- **No tool uses a per-channel tolerance.**
  - pixelmatch's `threshold` is a perceptual colour distance (default 0.1), with anti-aliased
    pixels detected and ignored by default [45]. v8 switched from YIQ to OKLab/HyAB [46].
  - Playwright's `toHaveScreenshot` uses `threshold` 0.2 (YIQ), with `maxDiffPixels` and
    `maxDiffPixelRatio` unset [47].
  - swift-snapshot-testing defaults to exact matching. Its `perceptualPrecision` maps to CIE Lab
    ΔE, where 0.98 means ΔE < 2, and a maintainer recommended 0.98–0.99 in 2022 [48][49].
  - By the YIQ arithmetic, 24/255 on a grey pixel is about pixelmatch threshold 0.09. It's looser
    for saturated hue shifts, and under the 0.1 default for grey [50]. That arithmetic is the
    worker's, from pixelmatch v7's source.
- **0.2% of pixels is too loose for small elements.** On a 3200×2000 window it allows 12,800
  pixels, which is a fully changed 113×113 px (56 pt) square. A missing 24 pt icon (2,304 px) or a
  1 pt border on one edge (6,400 px) passes [50]. No tool documents 0.2% as a default. The nearest
  published practice is 0.995 precision, i.e. 0.5%, with 0.98 perceptual [49].
- **Noise, measured here [local]:**
  - Two captures of the same static, on-screen window on the 2× display, a second apart, differed
    in **0 of 8,056,320 pixels**, whether taken by SCK or `screencapture`, in both runs.
  - A Finder window listed off-screen (1×, 1481×954) was different: two SCK stills a second apart
    differed in **1,810 pixels, by at most 3/255** per channel. Its `screencapture` and SCK stills
    matched each other exactly.
  - So on one machine, for an on-screen window on the 2× display, static content had no noise.
    That doesn't hold for every window: the off-screen 1× window had small, low-amplitude noise,
    and one pair of runs doesn't say where it came from. A tolerance of a few levels per channel
    (or ΔE 2, below) absorbs it; an exact-match rule wouldn't.
  - Noise comes from changing environments instead: a different OS or Xcode, Intel vs Apple
    Silicon, a CI VM's title-bar height [49][51], a different display (scale, ICC profile), and
    anything live (caret, clocks, animations, relative times).
- **My recommendation for mu-133:**
  - Against an **approved baseline** (G5), on the same Mac and display: normalise to sRGB, mask
    live regions, and fail on any pixel over ΔE 2 (or pixelmatch 0.1) once more than about 50 px
    differ. A ratio lets small elements slip through.
  - Store baselines keyed by OS version and scale factor.
  - Against a **mockup** (G6), a pixel diff is the wrong tool: the mockup wasn't rendered by the
    app's engine. Use layout checks (element present, bounds within N pt of the mockup's,
    contrast, no clipping), then the vision model on crops.
  - Neither the 24/255 nor the 0.2% figure holds up as written.

### 7. The walkthrough and what it leaves behind (added with the 19:43 scope)

- **Overlay:** `addInitScript` runs on every navigation and in every attached or navigated child
  frame, after the document exists and before its scripts; the order between context and page init
  scripts isn't defined [W1]. In the probe, the overlay injected on the context was back after a
  link navigation **[local]**.
- **Verdicts back to the runner:** `exposeBinding` adds a window function to every frame, survives
  navigations, and its callback gets `{ browserContext, page, frame }` as the first argument [W2].
  Both overlay buttons reached the Node runner with the page they came from **[local]**. The
  `handle` option was removed in 1.61 [W3].
- **Recording steps as locators:** codegen generates role, text and test-id locators first, records
  assertions from its toolbar, takes `--test-id-attribute`, `--save-storage` and `-b webkit`
  [W10][W11]. Turning the owner's own clicks into steps programmatically has only a private API:
  1.64 split `_enableRecorder` into `_showRecorder`, `_startRecording` and `_stopRecording` [W12],
  and in 1.64.1 the first is gone and the other three exist **[local]**. My recommendation: the
  overlay records clicks itself (an event listener that builds a role or test-id locator), and the
  private recorder isn't relied on.
- **Masks and stable captures:** `mask` paints chosen locators in `maskColor` (default `#FF00FF`,
  since 1.35), `stylePath` (1.41) hides more by CSS, and `toHaveScreenshot` disables animations and
  hides the caret by default [W4]. A mask follows the element's box at capture time: over a ticking
  clock, two masked shots were identical on one run and 38 pixels apart at the mask's edge on
  another. Freezing time with `page.clock.setFixedTime` made them identical **[local]**. Freeze
  time and seed data first, and mask only what can't be fixed.
- **Traces:** `tracing.start({ screenshots, snapshots, sources })` [W5]; the run's `trace.zip` was
  256–358 KB **[local]**. The trace viewer "loads the trace entirely in your browser and does not
  transmit any data externally" [W5]; `npx playwright show-trace` serves it locally, which fits
  the privacy rule.
- **Headed vs headless:** maintainers say the two modes can't be made pixel-identical [W13], so
  baselines come from the headless run and the walkthrough's screenshots are evidence, not
  baselines, until re-captured headless. Headed WebKit doesn't steal focus on current builds, per
  an open issue's author [W14].

### 8. Judging from video keyframes

- **Models take frames, not video,** except Gemini. Claude takes still images (animated GIFs use
  their first frame), up to 100 per request on 200k-context models, and caps each side at 2000 px
  once a request has more than 20 images [M7]. OpenAI's cookbook samples every 25th frame [M8].
  Gemini samples video at 1 fps by default [M9], but it's a cloud upload, so it's out here.
- **Picking frames:** ffmpeg's `select='gt(scene,X)'` keeps scene changes (the docs suggest 0.3 to
  0.5), `eq(pict_type,I)` or `-skip_frame nokey` keeps keyframes, `mpdecimate` drops near-duplicates,
  and `tile` makes a contact sheet [M10][M11]. UI recordings change less than film: on a 174-frame
  simulator clip, 0.1 kept one scene frame, plus the first and last **[local]** (`keyframes.sh`).
- **Why it matters:** GUI-World found models struggle with dynamic GUI content without annotated
  keyframes or the operation history [M12]. My recommendation: Kia judges from the frames at each
  scripted step (the runner knows when a step ends), plus scene-change frames between steps, sent
  as separate images at native resolution, as §5 says. The full clip is for the owner.

## Trade-offs / what would change the answer

- If Apple or Tauri ships a WKWebView WebDriver, driving the real Tauri app in a walkthrough gets simpler. Until then every
  route is an in-app debug plugin, which is a new dependency and needs the owner's yes and
  `/deps-audit vet`. WebdriverIO's is the best-maintained of the three; Choochmeque's is the
  smallest.
- If the Screen Recording re-prompt turns out to block unattended runs on 26.x, the fallback for
  stills is headless Playwright screenshots (web stacks) and snapshot tests (Mac), which need no
  TCC. Video then falls back to Playwright's 25 fps page recording, headless.
- If a frontier model passes ~80% recall on DiffSpot-style hard changes, the model can do more of
  the comparing. Its false-alarm rate is already low enough [24].

## Gaps and caveats

- **Q1:**
  - WebdriverIO lists screenshot capture for Windows and Linux. Its macOS section has no feature
    list [3], so screenshots there are unconfirmed.
  - None of the macOS WebDriver plugins was tested here against macOS 26, native menus or dialogs.
- **Q2:**
  - Video excluding native chrome is my inference (no source).
  - #43247 is open and 1.64.1 isn't said to fix it [13]. The renderer-in-Chromium agent route for
    choopan (a stub preload API) wasn't prototyped.
- **Q3:**
  - No primary Apple source says SwiftPM can't declare a UI-test target, and Xcode 26 wasn't
    re-checked [14].
  - The cdhash behaviour of Accessibility grants is community-sourced [23].
  - The `.accessibilityIdentifier` → `AXIdentifier` mapping and `XCUIApplication(bundleIdentifier:)`
    on macOS weren't confirmed from Apple docs.
  - The AX driver itself wasn't prototyped here. Under G4 it's a walkthrough tool, so mu-141
    prototypes it inside a walkthrough the owner starts.
- **Q4:**
  - The Screen Recording re-prompt interval on Tahoe isn't documented by Apple. The monthly
    figure is 2024 Sequoia reporting [35][36].
  - The PPPC and `tccutil` claims come from an MDM vendor's knowledge base and a forum repro step
    [33][34].
  - Which process is "responsible" for this Mac's Claude Code background job wasn't identified,
    because `ps` was blocked in the sandbox. It already holds Screen Recording **[local]**.
  - The signed capture helper wasn't built. That launching it through LaunchServices makes it its
    own responsible process rests on [22]'s rule, not on a test here; mu-141 checks it with
    `CGPreflightScreenCaptureAccess()` from the helper while the terminal has no grant.
- **Q5:**
  - The numbers are for web UIs and mobile apps, not macOS windows.
  - There's no published ablation on overlay or heatmap composites for UI pairs.
  - Design2Code correlations weren't used, because they came only from a search summary.
- **Q6:**
  - The YIQ weights in the 24/255 arithmetic are from memory of pixelmatch v7 [50].
  - Display P3 vs sRGB and GPU vs CPU noise weren't measured across machines. Only one Mac was
    tested.
- **Out of scope:** hosted visual testing services (Percy, Chromatic, Applitools). They upload
  screenshots to a vendor, and nothing leaves the Mac.
- **Added scope (19:43), not covered by a local test:** codegen with `-b webkit` on macOS (no
  failure reports, no doc saying it works) [W10]; XCUITest on the iOS simulator end to end, and
  whether it needs Automation Mode [S10]; whether a headless simulator ever needs a host TCC grant
  (none appeared here, and no source states it either way); Liquid Glass in a macOS snapshot [M4];
  AX scripting inside a walkthrough. mu-141 (iOS, Mac) picks these up; codegen and the overlay's
  own click recorder go to mu-133.
- **Off-Space capture:** the first run recorded a window on another Space; the rerun couldn't check
  one without moving a window, which G4 rules out. Nothing above depends on it.
- **Sources read through a summarising fetch:** some quotes for [W1]–[W14], [S2]–[S11] and
  [M1]–[M12] came back as summaries. The [local] results don't depend on them.

## Sources

Read 2026-10-14 unless noted. "Undated" means the page shows no date.

[1] WebDriver — Tauri, undated — https://v2.tauri.app/develop/tests/webdriver/ (primary)
[2] Browsers — Playwright, undated — https://playwright.dev/docs/browsers (primary)
[3] Platform Support (Tauri) — WebdriverIO, undated — https://webdriver.io/docs/desktop-testing/tauri/platform-support/ (primary)
[4] Plugin Setup — WebdriverIO, undated — https://webdriver.io/docs/desktop-testing/tauri/plugin-setup/ (primary)
[5] Choochmeque/tauri-plugin-webdriver — GitHub, last commit 2026-10-04 — https://github.com/Choochmeque/tauri-plugin-webdriver (primary)
[6] Playwright v1.64.0 release — Microsoft, 2026-10-07 — https://github.com/microsoft/playwright/releases/tag/v1.64.0 (primary)
[7] I Built a WebDriver for WKWebView Tauri Apps on macOS — Daniel Raffel, 2026-02-14 — https://danielraffel.me/2026/02/14/i-built-a-webdriver-for-wkwebview-tauri-apps-on-macos/ (secondary)
[8] Mock Tauri APIs — Tauri, undated — https://v2.tauri.app/develop/tests/mocking/ (primary)
[9] Electron class — Playwright, undated — https://playwright.dev/docs/api/class-electron (primary); Browser.newContext and Videos — https://playwright.dev/docs/api/class-browser#browser-new-context, https://playwright.dev/docs/videos (primary)
[10] Automated Testing — Electron, undated — https://www.electronjs.org/docs/latest/tutorial/automated-testing (primary)
[11] electron.launch() fails with Electron 30+ (#39008) — GitHub, 2026-01-28 — https://github.com/microsoft/playwright/issues/39008 (primary)
[12] Playwright Testing: WebKit Engine Mismatch — zudo-tauri-wisdom, undated — https://zudo-tauri-wisdom.takazudomodular.com/docs/frontend/playwright-engine-pitfall/ (secondary)
[12b] electron.launch() still fails on Electron 40.2.1 after revert (#39922) — GitHub, 2026-03-29 — https://github.com/microsoft/playwright/issues/39922 (primary)
[13] _electron.launch intermittently loses a packaged app's first window since 1.64.0 (#43247) — GitHub, 2026-10-09 — https://github.com/microsoft/playwright/issues/43247 (primary)
[14] How do you UI test a swift package? — Swift Forums, 2021-06-24 — https://forums.swift.org/t/how-do-you-ui-test-a-swift-package/49890 (secondary, stale)
[15] XcodeGen ProjectSpec — yonaskolb/XcodeGen, undated — https://github.com/yonaskolb/XcodeGen/blob/master/Docs/ProjectSpec.md (primary)
[16] Two Gates Guard Unattended macOS UI Tests — Corey Daley, 2026-06-09 — https://coreydaley.dev/posts/2026/06/unattended-xcuitest-self-hosted-mdm/ (secondary)
[17] testmanagerd constantly requests password to enable UI Automation — Apple Developer Forums (DTS), 2021-11 — https://developer.apple.com/forums/thread/693850 (primary, stale)
[18] Capture HDR content with ScreenCaptureKit — Apple WWDC24, 2024-06-10 — https://developer.apple.com/videos/play/wwdc2024/10088/ (primary)
[19] Electron Fuses — Electron, undated — https://www.electronjs.org/docs/latest/tutorial/fuses (primary)
[20] Spectron Deprecation Notice — Electron, 2022-01-26 — https://www.electronjs.org/blog/spectron-deprecation-notice (primary)
[21] WebdriverIO v10 release — WebdriverIO, 2026-10-05 — https://webdriver.io/blog/2026/10/05/webdriverio-v10-release (primary)
[22] How to grant command line tools full disk access — Apple Developer Forums (Quinn, DTS), 2024-06 — https://developer.apple.com/forums/thread/756510 (primary)
[23] Built-in updater breaks Accessibility permission for ad-hoc signed builds — GitHub shlgd/SuperDictate #19, undated — https://github.com/shlgd/SuperDictate/issues/19 (secondary)
[24] DiffSpot: Can VLMs Spot Fine-Grained Visual Differences in Web Interfaces? — arXiv 2605.29615, 2026-05-28 — https://arxiv.org/abs/2605.29615 (primary)
[25] macOS application screen recording — Apple Developer Forums (Quinn), 2021-12 — https://developer.apple.com/forums/thread/695689 (primary, stale)
[26] Mac2 driver Getting Started — Appium, undated — https://appium.github.io/appium-mac2-driver/latest/getting-started/ (primary)
[27] Peekaboo releases — openclaw/Peekaboo, v4.9.3 2026-10-14 — https://github.com/openclaw/Peekaboo/releases (primary)
[28] Peekaboo permissions — openclaw/Peekaboo, undated — https://github.com/openclaw/Peekaboo/blob/main/docs/permissions.md (primary)
[29] osascript/System Events automation fails from embedded terminal — GitHub stablyai/orca #12692, undated — https://github.com/stablyai/orca/issues/12692 (secondary)
[30] screencapture(1) — Apple man page (mirror), undated — https://keith.github.io/xcode-man-pages/screencapture.1.html (primary); also `man screencapture` on macOS 26.5 **[local]**
[31] Take correctly sized screenshots with ScreenCaptureKit — Apple Developer Forums (DTS), 2024-10 — https://developer.apple.com/forums/thread/765360 (primary)
[32] Privacy for XPC module — Apple Developer Forums (Quinn), 2022-05 — https://developer.apple.com/forums/thread/706187 (primary)
[33] Plain Executables Do Not Appear Under 'Screen & System Audio Recording' on macOS 26.1 — Apple Developer Forums, 2025-11 — https://developer.apple.com/forums/thread/807898 (primary)
[34] PPPC for Standard Users — Addigy, undated — https://support.addigy.com/hc/en-us/articles/4403549601043-Privacy-Preferences-Policy-Control-PPPC-for-Standard-Users (secondary)
[35] macOS Sequoia will now prompt you monthly for screen recording permissions — 9to5Mac, 2024-08-14 — https://9to5mac.com/2024/08/14/macos-sequoia-screen-recording-prompt-monthly/ (secondary)
[36] Apple Tweaks Screen Recording App Permissions in macOS Sequoia 15.1 — MacRumors, 2024-10-07 — https://www.macrumors.com/2024/10/07/apple-screen-recording-popup-update/ (secondary)
[37] ScreenCaptureKit persistent-content-capture entitlement? — Apple Developer Forums, 2024-08 — https://developer.apple.com/forums/thread/761641 (primary, thin)
[38] GUIPilot — arXiv 2506.07385 / ISSTA 2025, 2025-06-09 — https://arxiv.org/abs/2506.07385 (primary)
[39] GUIPilot full text, Table 3 — arXiv, 2025-06-09 — https://arxiv.org/html/2506.07385v1 (primary)
[40] VLM-SubtleBench — arXiv 2603.07888, 2026-03 — https://arxiv.org/html/2603.07888v1 (primary)
[41] Coordinates and bounding boxes — Anthropic, undated — https://platform.claude.com/docs/en/build-with-claude/vision-coordinates (primary)
[42] Images and vision — OpenAI, undated — https://developers.openai.com/api/docs/guides/images-vision (primary)
[43] WCAG 2.2, SC 1.4.3 Contrast (Minimum) — W3C, 2023-10-05 — https://www.w3.org/TR/WCAG22/#contrast-minimum (primary)
[44] Beyond Pixel Diffs: Image Change Captioning for Web UI Visual Regression Testing — arXiv 2607.01728, 2026-07-02 — https://arxiv.org/abs/2607.01728 (primary)
[45] mapbox/pixelmatch README — GitHub, undated — https://github.com/mapbox/pixelmatch (primary)
[46] pixelmatch releases (v8.0.0) — GitHub, undated — https://github.com/mapbox/pixelmatch/releases (primary)
[47] PageAssertions.toHaveScreenshot and Visual comparisons — Playwright, undated — https://playwright.dev/docs/api/class-pageassertions, https://playwright.dev/docs/test-snapshots (primary)
[48] swift-snapshot-testing NSImage.swift / UIImage.swift — pointfreeco, undated — https://github.com/pointfreeco/swift-snapshot-testing/blob/main/Sources/SnapshotTesting/Snapshotting/NSImage.swift (primary)
[49] Perceptual image precision (PR #628) — pointfreeco/swift-snapshot-testing, undated — https://github.com/pointfreeco/swift-snapshot-testing/pull/628 (primary)
[50] pixelmatch v7.1.0 index.js — GitHub — https://raw.githubusercontent.com/mapbox/pixelmatch/v7.1.0/index.js (primary; the threshold-to-colour arithmetic is derived)
[51] Our SwiftUI snapshot tests passed locally but failed on CI — DEV Community, undated — https://dev.to/daksh-gargas/our-swiftui-snapshot-tests-passed-locally-but-failed-on-ci-heres-the-actual-fix-5fhd (secondary, from a search summary)
[52] ScreenCaptureKit headers (`SCRecordingOutput.h`, `SCScreenshotManager.h`, `SCStream.h`) — macOS 26.5 SDK in Xcode 26.6 **[local]**

Added in round 1's revision (read 2026-10-14):

[W1] Page.addInitScript — Playwright, undated — https://playwright.dev/docs/api/class-page (primary)
[W2] BrowserContext.exposeBinding and Page.exposeBinding — Playwright, undated — https://playwright.dev/docs/api/class-browsercontext (primary)
[W3] Release notes (1.61 removed APIs) — Playwright, undated — https://playwright.dev/docs/release-notes (primary)
[W4] PageAssertions.toHaveScreenshot — Playwright, undated — https://playwright.dev/docs/api/class-pageassertions (primary)
[W5] Trace viewer, and Tracing.start — Playwright, undated — https://playwright.dev/docs/trace-viewer, https://playwright.dev/docs/api/class-tracing (primary)
[W7] Electron class — Playwright, undated — https://playwright.dev/docs/api/class-electron (primary)
[W8] videoRecorder.ts — microsoft/playwright main, undated — https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/server/videoRecorder.ts (primary)
[W9] webContents.capturePage — Electron, undated — https://www.electronjs.org/docs/latest/api/web-contents (primary)
[W10] Test generator — Playwright, undated — https://playwright.dev/docs/codegen (primary)
[W11] program.ts (codegen CLI options) — microsoft/playwright main, undated — https://github.com/microsoft/playwright/blob/main/packages/playwright-core/src/cli/program.ts (primary)
[W12] fix(recorder): split showRecorder and startRecording (#42627) — GitHub, 2026-09-09 — https://github.com/microsoft/playwright/pull/42627 (primary)
[W13] Chromium and webkit headless/head rendering differences (#12683) — GitHub, 2022-03-11, closed 2026-05-07 — https://github.com/microsoft/playwright/issues/12683 (primary)
[W14] Allow Chrome to launch in the background during headful mode (#41306) — GitHub, 2026-06-15 — https://github.com/microsoft/playwright/issues/41306 (primary, author's statement)
[S1] `xcrun simctl help io` — Xcode 26.6 **[local]** (primary)
[S2] A recording covers start to stop (#19) — olbartek/agentctl-ios, 2026-09-30 — https://github.com/olbartek/agentctl-ios/pull/19 (secondary)
[S3] simctl — NSHipster, 2018-11-26 — https://nshipster.com/simctl/ (secondary, stale)
[S4] Mocking Capabilities in the iOS Simulator — Saagar Jha, 2019-01-11 — https://saagarjha.com/blog/2019/01/11/mocking-capabilities-in-the-ios-simulator/ (secondary, stale)
[S6] xcresulttool(1) — keith/xcode-man-pages, undated — https://keith.github.io/xcode-man-pages/xcresulttool.1.html (primary, mirror)
[S7] UI Testing Swift Packages with a Runner App — Medium, undated — https://medium.com/@alisha.latash/ui-testing-swift-packages-with-a-runner-app-a6bafd78abcd (secondary)
[S8] simctl ui appearance fixture step is SIGKILLed on the macOS 26 runner (#7605) — kaeawc/auto-mobile, 2026-09-25 — https://github.com/kaeawc/auto-mobile/issues/7605 (secondary)
[S9] ci: stop booting the UI test simulator before xcodebuild starts (#173) — xchrisbailey/kyo, 2026-10-07 — https://github.com/xchrisbailey/kyo/pull/173 (secondary)
[S10] Shepherd issue #2695 — erwins-enkel/shepherd, undated — https://github.com/erwins-enkel/shepherd/issues/2695 (secondary, thin)
[S11] simctl-commands.md — inekipelov/apple-platform-skills, undated — https://github.com/inekipelov/apple-platform-skills/blob/main/skills/ios-simulator/references/simctl-commands.md (secondary)
[M1] swift-snapshot-testing 1.19.6 — pointfreeco, 2026-09-21 — https://github.com/pointfreeco/swift-snapshot-testing/releases/tag/1.19.6 (primary)
[M2] swift-snapshot-testing README, NSView.swift, SwiftUIView.swift — pointfreeco, undated — https://github.com/pointfreeco/swift-snapshot-testing (primary)
[M3] ImageRenderer — Apple, undated — https://developer.apple.com/documentation/swiftui/imagerenderer (primary)
[M4] Snapshot containing SwiftUI liquid glass effect is fully transparent (#1031) — pointfreeco, 2025-10-03 — https://github.com/pointfreeco/swift-snapshot-testing/discussions/1031 (primary)
[M5] How to snapshot a view on macOS with provided screen scale? (#428) — pointfreeco, undated — https://github.com/pointfreeco/swift-snapshot-testing/issues/428 (secondary, from a summary)
[M7] Vision — Anthropic, undated — https://platform.claude.com/docs/en/build-with-claude/vision (primary)
[M8] Processing and narrating a video with GPT's visual capabilities — OpenAI Cookbook, undated (archived) — https://developers.openai.com/cookbook/examples/gpt_with_vision_for_video_understanding (primary)
[M9] Video understanding — Google, 2026-10-13 — https://ai.google.dev/gemini-api/docs/video-understanding (primary)
[M10] FFmpeg filters (mpdecimate) and codecs (skip_frame) — FFmpeg, undated — https://ffmpeg.org/ffmpeg-filters.html, https://ffmpeg.org/ffmpeg-codecs.html (primary)
[M11] select and tile filters — FFmpeg 4.1 docs mirror, undated — https://ayosec.github.io/ffmpeg-filters-docs/4.1/Filters/Multimedia/select.html (secondary, mirror)
[M12] GUI-World — arXiv 2406.10819, 2024-06-16 — https://arxiv.org/abs/2406.10819 (primary)
