import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    self.contentViewController = flutterViewController

    self.minSize = NSSize(width: 700, height: 400)
    if let screen = NSScreen.main {
      let vf = screen.visibleFrame
      let w = min(1240, max(920, vf.width - 80))
      let h = min(860, max(640, vf.height - 80))
      let x = vf.origin.x + (vf.width - w) / 2
      let y = vf.origin.y + (vf.height - h) / 2
      setFrame(NSRect(x: x, y: y, width: w, height: h), display: true)
    }

    RegisterGeneratedPlugins(registry: flutterViewController)

    // Register here (not only in AppDelegate): mainFlutterWindow can be nil when
    // applicationDidFinishLaunching’s async block runs, so the channel was never set up.
    let dockBadgeChannel = FlutterMethodChannel(
      name: "dev.tagliacarte/dock_badge",
      binaryMessenger: flutterViewController.engine.binaryMessenger
    )
    dockBadgeChannel.setMethodCallHandler { call, result in
      if call.method == "setBadge" {
        let label = (call.arguments as? [String: Any?])?["label"] as? String
        let trimmed = label?.trimmingCharacters(in: .whitespacesAndNewlines)
        if let t = trimmed, !t.isEmpty {
          NSApplication.shared.dockTile.badgeLabel = t
        } else {
          NSApplication.shared.dockTile.badgeLabel = nil
        }
        NSApplication.shared.dockTile.display()
        result(nil)
      } else {
        result(FlutterMethodNotImplemented)
      }
    }

    super.awakeFromNib()
  }
}
