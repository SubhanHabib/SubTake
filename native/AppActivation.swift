import AppKit

// App-wide state that is not tied to one window: the Dock icon and whether
// SubTake shows in the Dock and app switcher at all.

@_cdecl("subtake_set_app_icon")
public func setAppIcon(_ bytes: UnsafePointer<UInt8>?, _ length: UInt) {
    guard let bytes else { return }
    let data = Data(bytes: bytes, count: Int(length))
    if let image = NSImage(data: data) {
        NSApp?.applicationIconImage = image
    }
}

/// The editor makes SubTake a regular app; with only the recorder open it is
/// an accessory that lives in the menu bar.
@_cdecl("subtake_set_editor_active")
public func setEditorActive(_ active: Bool) {
    NSApp?.setActivationPolicy(active ? .regular : .accessory)
    if active {
        NSApp?.activate(ignoringOtherApps: true)
    }
}

/// Activates the app without making every floating surface key. The recorder
/// bar owns its options child, and AppKit preserves that child's ordering and
/// focus.
@_cdecl("subtake_activate_launcher")
public func activateLauncher() {
    NSApp?.activate(ignoringOtherApps: true)
}
