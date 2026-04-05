import Cocoa
import FlutterMacOS

@main
class AppDelegate: FlutterAppDelegate, NSUserInterfaceValidations {
  private var mailMenuChannel: FlutterMethodChannel?
  /// Keys match Flutter `mailAction` / `_handleMacMailMenuIntent` (e.g. `reply-all`).
  private var mailMenuEnabled: [String: Bool] = [:]

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
    let channel = FlutterMethodChannel(
      name: "dev.tagliacarte/mail_menu",
      binaryMessenger: controller.engine.binaryMessenger
    )
    mailMenuChannel = channel
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self else {
        result(FlutterError(code: "dead", message: "AppDelegate released", details: nil))
        return
      }
      if call.method == "setMailMenuState" {
        if let raw = call.arguments as? [String: Any] {
          var next: [String: Bool] = [:]
          for (k, v) in raw {
            if let b = v as? Bool {
              next[k] = b
            }
          }
          self.mailMenuEnabled = next
          NSApp.mainMenu?.items.forEach { item in
            item.submenu?.items.forEach { sub in
              sub.submenu?.items.forEach { leaf in
                leaf.isEnabled = self.validateUserInterfaceItem(leaf)
              }
            }
          }
        }
        result(nil)
        return
      }
      result(FlutterMethodNotImplemented)
    }
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

  func validateUserInterfaceItem(_ item: NSValidatedUserInterfaceItem) -> Bool {
    guard let menuItem = item as? NSMenuItem, let action = menuItem.action else {
      return true
    }
    let key: String?
    switch action {
    case #selector(tagliacarteMailCompose(_:)):
      key = "compose"
    case #selector(tagliacarteMailReply(_:)):
      key = "reply"
    case #selector(tagliacarteMailReplyAll(_:)):
      key = "reply-all"
    case #selector(tagliacarteMailForward(_:)):
      key = "forward"
    case #selector(tagliacarteMailDelete(_:)):
      key = "delete"
    case #selector(tagliacarteMailJunk(_:)):
      key = "junk"
    case #selector(tagliacarteMailMove(_:)):
      key = "move"
    case #selector(tagliacarteMailCopy(_:)):
      key = "copy"
    default:
      key = nil
    }
    if let key, let v = mailMenuEnabled[key] {
      return v
    }
    return true
  }
}
