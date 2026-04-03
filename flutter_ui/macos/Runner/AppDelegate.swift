import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate {
  private var mailMenuChannel: FlutterMethodChannel?

  override func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
    return true
  }

  override func applicationSupportsSecureRestorableState(_ app: NSApplication) -> Bool {
    return true
  }

  override func applicationDidFinishLaunching(_ notification: Notification) {
    super.applicationDidFinishLaunching(notification)
    DispatchQueue.main.async { [weak self] in
      self?.attachMailMenuChannel()
    }
  }

  private func attachMailMenuChannel() {
    guard mailMenuChannel == nil,
          let window = mainFlutterWindow,
          let controller = window.contentViewController as? FlutterViewController else {
      return
    }
    mailMenuChannel = FlutterMethodChannel(
      name: "dev.tagliacarte/mail_menu",
      binaryMessenger: controller.engine.binaryMessenger
    )
  }

  private func invokeMailAction(_ action: String) {
    if mailMenuChannel == nil {
      attachMailMenuChannel()
    }
    mailMenuChannel?.invokeMethod("mailAction", arguments: ["action": action])
  }

  @objc func tagliacarteMailCompose(_ sender: Any?) {
    invokeMailAction("compose")
  }

  @objc func tagliacarteMailReply(_ sender: Any?) {
    invokeMailAction("reply")
  }

  @objc func tagliacarteMailReplyAll(_ sender: Any?) {
    invokeMailAction("reply-all")
  }

  @objc func tagliacarteMailForward(_ sender: Any?) {
    invokeMailAction("forward")
  }

  @objc func tagliacarteMailDelete(_ sender: Any?) {
    invokeMailAction("delete")
  }

  @objc func tagliacarteMailJunk(_ sender: Any?) {
    invokeMailAction("junk")
  }

  @objc func tagliacarteMailMove(_ sender: Any?) {
    invokeMailAction("move")
  }

  @objc func tagliacarteMailCopy(_ sender: Any?) {
    invokeMailAction("copy")
  }
}
