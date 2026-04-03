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

    super.awakeFromNib()
  }
}
