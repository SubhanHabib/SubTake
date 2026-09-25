import AppKit

/// Receives whether the pointer is over the recorder bar.
public typealias LauncherHoverCallback = @convention(c) (Bool) -> Void

/// Tells the bar when the pointer comes onto it and, `linger` after it
/// leaves, that it has gone. GPUI follows the pointer only in the key
/// window of the active app, and while a capture runs the app being
/// recorded is the active one, so the bar watches for itself.
final class LauncherHover: NSResponder {
    let callback: LauncherHoverCallback
    let linger: TimeInterval
    /// Each crossing of the edge, so a leave that is followed by a return
    /// within `linger` is not reported.
    var crossings = 0

    init(_ callback: @escaping LauncherHoverCallback, linger: TimeInterval) {
        self.callback = callback
        self.linger = linger
        super.init()
    }

    required init?(coder: NSCoder) { nil }

    override func mouseEntered(with event: NSEvent) {
        crossings += 1
        callback(true)
    }

    override func mouseExited(with event: NSEvent) {
        crossings += 1
        let crossing = crossings
        DispatchQueue.main.asyncAfter(deadline: .now() + linger) { [weak self] in
            guard let self, self.crossings == crossing else { return }
            self.callback(false)
        }
    }
}

private let hoverKey = associationKey()

/// Watches the pointer over the recorder bar's window for `callback`, the
/// leave `linger` milliseconds late. Watching already, it does nothing.
@_cdecl("subtake_watch_launcher_hover")
public func watchLauncherHover(_ pointer: UnsafeMutableRawPointer?, _ callback: LauncherHoverCallback?, _ linger: UInt32) {
    assert(Thread.isMainThread, "The recorder bar must be watched on the UI thread")
    guard let callback, let view = borrowedView(pointer), let window = view.window,
          objc_getAssociatedObject(view, hoverKey) == nil
    else { return }
    let hover = LauncherHover(callback, linger: TimeInterval(linger) / 1000)
    objc_setAssociatedObject(view, hoverKey, hover, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    view.addTrackingArea(NSTrackingArea(
        rect: .zero,
        options: [.mouseEnteredAndExited, .activeAlways, .inVisibleRect],
        owner: hover,
        userInfo: nil
    ))
    callback(window.frame.contains(NSEvent.mouseLocation))
}

/// Makes the recorder bar's window `width` points wide about its centre,
/// so the bar grows and shrinks in place wherever it was dragged.
@_cdecl("subtake_set_launcher_width")
public func setLauncherWidth(_ pointer: UnsafeMutableRawPointer?, _ width: Double) {
    assert(Thread.isMainThread, "The recorder bar must be resized on the UI thread")
    guard let window = borrowedView(pointer)?.window, width > 0 else { return }
    var frame = window.frame
    let width = CGFloat(width)
    guard abs(frame.width - width) >= 0.5 else { return }
    frame.origin.x += (frame.width - width) / 2
    frame.size.width = width
    window.setFrame(frame, display: true)
}
