import AppKit

// The runtime's window bridge. Every call needs the UI thread and a live
// borrowed GPUI view. GPUI's view and window delegate stay intact, so its
// resize, input, focus and close callbacks keep running. Sizes are points.

@_cdecl("subtake_window_number")
public func windowNumber(_ view: UnsafeMutableRawPointer?) -> UInt32 {
    guard let number = borrowedWindow(view)?.windowNumber, number > 0, number <= Int(UInt32.max) else {
        return 0
    }
    return UInt32(number)
}

@_cdecl("subtake_window_show")
public func windowShow(_ view: UnsafeMutableRawPointer?) {
    borrowedWindow(view)?.orderFront(nil)
}

@_cdecl("subtake_window_hide")
public func windowHide(_ view: UnsafeMutableRawPointer?) {
    guard let window = borrowedWindow(view) else { return }

    // A child that had focus, the recorder card, hands it back to the window
    // it hangs from, the bar, rather than to whatever AppKit picks.
    let parent = window.isKeyWindow ? window.parent : nil
    window.orderOut(nil)
    parent?.makeKey()
}

/// Focus without activating the app or reordering its windows.
@_cdecl("subtake_window_make_key")
public func windowMakeKey(_ view: UnsafeMutableRawPointer?) {
    borrowedWindow(view)?.makeKey()
}

@_cdecl("subtake_window_focus")
public func windowFocus(_ view: UnsafeMutableRawPointer?) {
    guard let window = borrowedWindow(view) else { return }
    NSApp?.activate(ignoringOtherApps: true)
    window.makeKeyAndOrderFront(nil)
}

@_cdecl("subtake_window_minimize")
public func windowMinimize(_ view: UnsafeMutableRawPointer?, _ minimized: Bool) {
    guard let window = borrowedWindow(view) else { return }
    if minimized {
        window.miniaturize(nil)
    } else {
        window.deminiaturize(nil)
    }
}

@_cdecl("subtake_window_set_transparent")
public func windowSetTransparent(_ view: UnsafeMutableRawPointer?, _ transparent: Bool) {
    guard let window = borrowedWindow(view) else { return }
    window.isOpaque = !transparent
    window.backgroundColor = transparent ? .clear : .windowBackgroundColor
}

/// Position is the outer frame's top-left in points, relative to the primary
/// display's top-left, with y growing downwards. Negative values are valid.
@_cdecl("subtake_window_set_position")
public func windowSetPosition(_ view: UnsafeMutableRawPointer?, _ x: Double, _ y: Double) {
    guard let window = borrowedWindow(view), let primary = NSScreen.screens.first,
          x.isFinite, y.isFinite
    else { return }
    window.setFrameTopLeftPoint(NSPoint(x: primary.frame.minX + x, y: primary.frame.maxY - y))
}

@_cdecl("subtake_window_get_position")
public func windowGetPosition(
    _ view: UnsafeMutableRawPointer?,
    _ x: UnsafeMutablePointer<Double>?,
    _ y: UnsafeMutablePointer<Double>?
) -> Bool {
    guard let window = borrowedWindow(view), let primary = NSScreen.screens.first,
          let x, let y
    else { return false }
    x.pointee = window.frame.minX - primary.frame.minX
    y.pointee = primary.frame.maxY - window.frame.maxY
    return true
}

/// Starts a system window drag from the mouse event being handled, if it is a
/// left press or drag on this window.
@_cdecl("subtake_window_drag")
public func windowDrag(_ view: UnsafeMutableRawPointer?) -> Bool {
    guard let window = borrowedWindow(view), let event = NSApp?.currentEvent,
          event.window === window,
          event.type == .leftMouseDown || event.type == .leftMouseDragged
    else { return false }
    window.performDrag(with: event)
    return true
}
