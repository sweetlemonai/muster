// mu-130 probe: renders a SwiftUI view to PNG off-screen, the way a Mac snapshot test does, and
// checks that no window of this process reached the WindowServer.
//
// Build: swiftc -O -parse-as-library -o "$TMPDIR/snapshot-probe" snapshot-probe.swift
// Run:   snapshot-probe <out-dir>
// Writes hosting-light.png, hosting-dark.png (NSHostingView + cacheDisplay, as swift-snapshot-testing
// does for NSView) and renderer.png (SwiftUI ImageRenderer), then prints this process's window count.
import AppKit
import SwiftUI

struct Card: View {
    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("mu-130 snapshot probe").font(.headline)
            Text("Synthetic fixture text, rendered off-screen.").foregroundStyle(.secondary)
            HStack {
                Button("Pass") {}
                Button("Fail") {}
                Toggle("Dark", isOn: .constant(true))
            }
            RoundedRectangle(cornerRadius: 8).fill(.blue).frame(height: 24)
            Rectangle().fill(.regularMaterial).frame(height: 24).overlay(Text("material"))
        }
        .padding(16)
        .frame(width: 360)
    }
}

@MainActor
func writePNG(_ rep: NSBitmapImageRep, _ url: URL) {
    guard let data = rep.representation(using: .png, properties: [:]) else {
        FileHandle.standardError.write(Data("can't encode \(url.lastPathComponent)\n".utf8))
        exit(1)
    }
    do { try data.write(to: url) } catch {
        FileHandle.standardError.write(Data("can't write \(url.path): \(error)\n".utf8))
        exit(1)
    }
    print("\(url.lastPathComponent) \(rep.pixelsWide)x\(rep.pixelsHigh)")
}

@MainActor
func hostingSnapshot(appearance: NSAppearance.Name, to url: URL) {
    let view = NSHostingView(rootView: Card())
    view.appearance = NSAppearance(named: appearance)
    view.frame = CGRect(origin: .zero, size: view.fittingSize)
    view.layoutSubtreeIfNeeded()
    guard let rep = view.bitmapImageRepForCachingDisplay(in: view.bounds) else {
        FileHandle.standardError.write(Data("no bitmap for \(appearance.rawValue)\n".utf8))
        exit(1)
    }
    view.cacheDisplay(in: view.bounds, to: rep)
    writePNG(rep, url)
}

@MainActor
func rendererSnapshot(to url: URL) {
    let renderer = ImageRenderer(content: Card())
    renderer.scale = 2
    guard let image = renderer.cgImage else {
        FileHandle.standardError.write(Data("ImageRenderer returned nil\n".utf8))
        exit(1)
    }
    writePNG(NSBitmapImageRep(cgImage: image), url)
}

func windowsOfThisProcess() -> Int {
    let all = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as? [[String: Any]] ?? []
    let pid = Int(ProcessInfo.processInfo.processIdentifier)
    return all.filter { ($0[kCGWindowOwnerPID as String] as? Int) == pid }.count
}

@main
struct SnapshotProbe {
    @MainActor
    static func main() {
        guard CommandLine.arguments.count == 2 else {
            FileHandle.standardError.write(Data("usage: snapshot-probe <out-dir>\n".utf8))
            exit(2)
        }
        let dir = URL(fileURLWithPath: CommandLine.arguments[1], isDirectory: true)
        // Accessory policy: no Dock icon, no menu bar, never activated.
        NSApplication.shared.setActivationPolicy(.prohibited)
        hostingSnapshot(appearance: .aqua, to: dir.appendingPathComponent("hosting-light.png"))
        hostingSnapshot(appearance: .darkAqua, to: dir.appendingPathComponent("hosting-dark.png"))
        rendererSnapshot(to: dir.appendingPathComponent("renderer.png"))
        print("windows owned by this process: \(windowsOfThisProcess())")
    }
}
