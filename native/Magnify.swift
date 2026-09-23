import AppKit

/// Receives a trackpad pinch: the view, the point in view coordinates with y
/// downwards, the magnification delta, and the raw `NSEvent.Phase` bits.
public typealias MagnifyCallback = @convention(c) (UnsafeMutableRawPointer?, Float, Float, Float, UInt8) -> Void

private let magnifyKey = associationKey()

/// One view's pinch registration. The view owns it through an associated
/// object. Neither it nor either AppKit block retains the view or the window,
/// so closing a window cannot leak GPUI's surface.
final class MagnifyMonitor: NSObject {
    weak var view: NSView?
    weak var window: NSWindow?
    var callback: MagnifyCallback?
    var monitor: Any?
    var closeObserver: NSObjectProtocol?

    func invalidate() {
        callback = nil
        if let monitor {
            NSEvent.removeMonitor(monitor)
            self.monitor = nil
        }
        if let closeObserver {
            NotificationCenter.default.removeObserver(closeObserver)
            self.closeObserver = nil
        }
    }

    deinit {
        if let monitor { NSEvent.removeMonitor(monitor) }
        if let closeObserver { NotificationCenter.default.removeObserver(closeObserver) }
    }

    func handle(_ event: NSEvent) -> NSEvent {
        // Hold strong references for the synchronous callback. The callback may
        // remove or replace this registration reentrantly.
        guard let view, let window, view.window === window else {
            invalidate()
            return event
        }
        guard let callback, event.type == .magnify, event.window === window,
              window.isVisible, !view.isHiddenOrHasHiddenAncestor
        else { return event }

        let point = view.convert(event.locationInWindow, from: nil)
        let bounds = view.bounds
        let x = Float(point.x - bounds.minX)
        let y = Float(view.isFlipped ? point.y - bounds.minY : bounds.maxY - point.y)
        let delta = Float(event.magnification)

        // Keep the phase bit mask, including zero-delta end and cancel events.
        // Coordinates are not clipped: a gesture can end outside the view.
        if x.isFinite, y.isFinite, delta.isFinite {
            let phase = UInt8(truncatingIfNeeded: event.phase.rawValue)
            callback(Unmanaged.passUnretained(view).toOpaque(), x, y, delta, phase)
        }

        // GPUI and the native responder chain still receive the event.
        return event
    }
}

@_cdecl("subtake_window_remove_magnify")
public func windowRemoveMagnify(_ pointer: UnsafeMutableRawPointer?) {
    assert(Thread.isMainThread, "Magnify registration must run on the UI thread")
    guard let view = borrowedView(pointer) else { return }
    (objc_getAssociatedObject(view, magnifyKey) as? MagnifyMonitor)?.invalidate()
    objc_setAssociatedObject(view, magnifyKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
}

@_cdecl("subtake_window_install_magnify")
public func windowInstallMagnify(_ pointer: UnsafeMutableRawPointer?, _ callback: MagnifyCallback?) -> Bool {
    guard let window = borrowedWindow(pointer), let view = borrowedView(pointer), let callback else {
        return false
    }
    windowRemoveMagnify(pointer)

    let registration = MagnifyMonitor()
    registration.view = view
    registration.window = window
    registration.callback = callback
    registration.monitor = NSEvent.addLocalMonitorForEvents(matching: .magnify) { [weak registration] event in
        registration?.handle(event) ?? event
    }
    guard registration.monitor != nil else { return false }

    registration.closeObserver = NotificationCenter.default.addObserver(
        forName: NSWindow.willCloseNotification,
        object: window,
        queue: nil
    ) { [weak registration] _ in
        registration?.invalidate()
    }

    objc_setAssociatedObject(view, magnifyKey, registration, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    return true
}
