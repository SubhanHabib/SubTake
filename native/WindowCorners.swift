import AppKit

// The editor's corners are the window's own. AppKit keeps a private corner
// radius on every titled window, 16 by default on macOS 27, and the window
// server clips the content, the shadow and the outline to that one shape.
// Setting it is the whole job. Nothing inside the window rounds or masks, so
// no layer can disagree with the window about where its corners are, and no
// rounded clip over the whole layer tree forces an offscreen pass each frame.
// AppKit squares the corners in full screen and restores them on the way out.

private let setRadius = NSSelectorFromString("_setCornerRadius:")

private let getRadius = NSSelectorFromString("_cornerRadius")

private let systemRadiusKey = associationKey()

private typealias RadiusGetter = @convention(c) (NSWindow, Selector) -> CGFloat

private typealias RadiusSetter = @convention(c) (NSWindow, Selector, CGFloat) -> Void

/// The radius `window` is drawn with, or nil where AppKit no longer has one.
func windowCornerRadius(_ window: NSWindow) -> CGFloat? {
    guard window.responds(to: getRadius) else { return nil }
    return unsafeBitCast(window.method(for: getRadius), to: RadiusGetter.self)(window, getRadius)
}

/// Rounds a titled window's corners at `radius` points, or hands them back to
/// the system at 0. An AppKit without the private radius keeps its own
/// corners, which is the right way for this to fail.
@_cdecl("subtake_window_set_corner_radius")
public func subtake_window_set_corner_radius(_ view: UnsafeMutableRawPointer?, _ radius: Double) {
    guard let window = borrowedWindow(view), window.responds(to: setRadius),
          let current = windowCornerRadius(window)
    else { return }

    // The first call remembers what the system chose, so 0 can restore it.
    if objc_getAssociatedObject(window, systemRadiusKey) == nil {
        let system = NSNumber(value: Double(current))
        objc_setAssociatedObject(window, systemRadiusKey, system, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    }
    let system = (objc_getAssociatedObject(window, systemRadiusKey) as? NSNumber)?.doubleValue
    let wanted = CGFloat(radius > 0 ? radius : system ?? Double(current))
    guard wanted != current else { return }
    unsafeBitCast(window.method(for: setRadius), to: RadiusSetter.self)(window, setRadius, wanted)
}
