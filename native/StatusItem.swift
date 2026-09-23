import AppKit

/// The menu-bar item: the brand mark with "Open" and "Quit", each reported to
/// Rust as an action string.
final class StatusMenuController: NSObject {
    var callback: StringCallback?

    @objc func open(_ sender: Any?) {
        send("show", to: callback)
    }

    @objc func quit(_ sender: Any?) {
        send("quit", to: callback)
    }
}

private var statusItem: NSStatusItem?
private let statusController = StatusMenuController()

@_cdecl("subtake_install_status_item")
public func installStatusItem(_ callback: StringCallback?) {
    statusController.callback = callback
    guard statusItem == nil else { return }

    let item = NSStatusBar.system.statusItem(withLength: NSStatusItem.squareLength)
    if let button = item.button {
        button.toolTip = "SubTake"
        button.image = menuBarImage()
    }

    let menu = NSMenu(title: "SubTake")
    menu.addItem(
        NSMenuItem(title: "Open SubTake", action: #selector(StatusMenuController.open(_:)), keyEquivalent: "")
    )
    menu.addItem(.separator())
    menu.addItem(
        NSMenuItem(title: "Quit SubTake", action: #selector(StatusMenuController.quit(_:)), keyEquivalent: "q")
    )
    for entry in menu.items {
        entry.target = statusController
    }

    item.menu = menu
    statusItem = item
}
