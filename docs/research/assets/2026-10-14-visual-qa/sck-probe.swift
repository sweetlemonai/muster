// mu-130 probe: finds, captures and records one app window with ScreenCaptureKit, and diffs PNGs.
// It never clicks, types or raises a window: it only reads windows that are already open.
//
// Build: swiftc -O -o "$TMPDIR/sck-probe" sck-probe.swift
// Run it outside Claude Code's Bash sandbox (inside it, CoreGraphics can't reach the WindowServer).
//
//   sck-probe list                       windows at layer 0: id, owner pid, owner, on-screen, bounds
//   sck-probe shot <window-id> <out.png> window-only still, Retina pixels, no shadow
//   sck-probe rec <window-id> <out.mp4> [secs]  window-only H.264 video, 30 fps cap
//   sck-probe diff <a.png> <b.png>       pixels whose largest per-channel delta is > 0 and > 24
import AppKit
import AVFoundation
import CoreGraphics
import Foundation
import ImageIO
import ScreenCaptureKit
import UniformTypeIdentifiers

func fail(_ message: String) -> Never {
    FileHandle.standardError.write(Data((message + "\n").utf8))
    exit(1)
}

// Draws the PNG into an sRGB RGBA buffer, so two captures compare in one colour space.
func loadRGBA(_ path: String) -> (width: Int, height: Int, pixels: [UInt8]) {
    guard let source = CGImageSourceCreateWithURL(URL(fileURLWithPath: path) as CFURL, nil),
          let image = CGImageSourceCreateImageAtIndex(source, 0, nil) else { fail("can't read \(path)") }
    let width = image.width, height = image.height
    var pixels = [UInt8](repeating: 0, count: width * height * 4)
    let context = CGContext(
        data: &pixels, width: width, height: height, bitsPerComponent: 8, bytesPerRow: width * 4,
        space: CGColorSpace(name: CGColorSpace.sRGB)!,
        bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
    context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
    return (width, height, pixels)
}

func diff(_ a: String, _ b: String) {
    let first = loadRGBA(a), second = loadRGBA(b)
    guard first.width == second.width, first.height == second.height else {
        print("size differs: \(first.width)x\(first.height) vs \(second.width)x\(second.height)")
        return
    }
    var histogram = [Int](repeating: 0, count: 256)
    for i in stride(from: 0, to: first.pixels.count, by: 4) {
        var delta = 0
        for channel in 0..<3 { delta = max(delta, abs(Int(first.pixels[i + channel]) - Int(second.pixels[i + channel]))) }
        histogram[delta] += 1
    }
    let total = first.width * first.height
    let differing = total - histogram[0]
    let over24 = histogram[25...].reduce(0, +)
    let maxDelta = histogram.lastIndex(where: { $0 > 0 }) ?? 0
    print("pixels \(total) differing \(differing) over24 \(over24) maxdelta \(maxDelta)")
}

func list() {
    // Window ids, owner pid and bounds need no Screen Recording grant; names do.
    let windows = CGWindowListCopyWindowInfo([.optionAll, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] ?? []
    print("screen-recording preflight: \(CGPreflightScreenCaptureAccess())")
    for window in windows where (window[kCGWindowLayer as String] as? Int) == 0 {
        let id = window[kCGWindowNumber as String] ?? "?"
        let pid = window[kCGWindowOwnerPID as String] ?? "?"
        let owner = window[kCGWindowOwnerName as String] ?? "?"
        let onScreen = window[kCGWindowIsOnscreen as String] as? Bool ?? false
        let bounds = window[kCGWindowBounds as String] as? [String: Any] ?? [:]
        print("\(id)\tpid \(pid)\t\(owner)\ton-screen \(onScreen)\t\(bounds["Width"] ?? "?")x\(bounds["Height"] ?? "?")")
    }
}

final class RecordingDelegate: NSObject, SCRecordingOutputDelegate, @unchecked Sendable {
    let finished = DispatchSemaphore(value: 0)
    // Set before `finished` is signalled and read only after the wait, so the semaphore orders it.
    private(set) var failure: Error?
    func recordingOutputDidFinishRecording(_ output: SCRecordingOutput) { finished.signal() }
    func recordingOutput(_ output: SCRecordingOutput, didFailWithError error: Error) {
        failure = error
        finished.signal()
    }
}

func capture(mode: String, windowID: CGWindowID, out: String, seconds: Double) async throws {
    // onScreenWindowsOnly false: a window on another Space is listed too.
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
    guard let window = content.windows.first(where: { $0.windowID == windowID }) else { fail("no window \(windowID)") }
    let filter = SCContentFilter(desktopIndependentWindow: window)
    let config = SCStreamConfiguration()
    config.width = Int(filter.contentRect.width * CGFloat(filter.pointPixelScale))
    config.height = Int(filter.contentRect.height * CGFloat(filter.pointPixelScale))
    config.showsCursor = false
    config.ignoreShadowsSingleWindow = true
    print("window \(window.owningApplication?.applicationName ?? "?") on-screen \(window.isOnScreen) "
        + "scale \(filter.pointPixelScale) -> \(config.width)x\(config.height)")
    if mode == "shot" {
        let image = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: config)
        guard let destination = CGImageDestinationCreateWithURL(
            URL(fileURLWithPath: out) as CFURL, UTType.png.identifier as CFString, 1, nil) else { fail("can't write \(out)") }
        CGImageDestinationAddImage(destination, image, nil)
        guard CGImageDestinationFinalize(destination) else { fail("can't write \(out)") }
        print("colour space \(image.colorSpace?.name as String? ?? "none")")
    } else {
        config.minimumFrameInterval = CMTime(value: 1, timescale: 30)
        let recording = SCRecordingOutputConfiguration()
        recording.outputURL = URL(fileURLWithPath: out)
        recording.outputFileType = .mp4
        recording.videoCodecType = .h264
        let delegate = RecordingDelegate()
        let output = SCRecordingOutput(configuration: recording, delegate: delegate)
        let stream = SCStream(filter: filter, configuration: config, delegate: nil)
        try stream.addRecordingOutput(output)
        try await stream.startCapture()
        try await Task.sleep(nanoseconds: UInt64(seconds * 1_000_000_000))
        try await stream.stopCapture()
        // The file is complete only after the delegate's finish callback; wait off the async context.
        let finished = await withCheckedContinuation { (resume: CheckedContinuation<Bool, Never>) in
            DispatchQueue.global().async {
                resume.resume(returning: delegate.finished.wait(timeout: .now() + 5) == .success)
            }
        }
        // A failed or unfinished recording exits non-zero, so a caller's `|| echo` sees it.
        if let error = delegate.failure { fail("recording failed: \(error)") }
        if !finished { fail("recording didn't finish within 5 s") }
    }
}

let args = CommandLine.arguments
// A bare CLI asserts CGS_REQUIRE_INIT in ScreenCaptureKit without a CoreGraphics connection first.
_ = NSApplication.shared
switch (args.count > 1 ? args[1] : "", args.count) {
case ("list", _):
    list()
case ("diff", 4):
    diff(args[2], args[3])
case ("shot", 4), ("rec", 4), ("rec", 5):
    guard let id = UInt32(args[2]) else { fail("window id must be a number") }
    let seconds = args.count == 5 ? Double(args[4]) ?? 3 : 3
    let done = DispatchSemaphore(value: 0)
    Task {
        do { try await capture(mode: args[1], windowID: id, out: args[3], seconds: seconds) } catch { fail("error: \(error)") }
        done.signal()
    }
    done.wait()
default:
    fail("usage: sck-probe list | shot <id> <out.png> | rec <id> <out.mp4> [secs] | diff <a.png> <b.png>")
}
