import AppKit

// The recorder's floating windows: the bar, the options card that hangs above
// it, and the full-screen countdown. GPUI keeps ownership of each content
// view; AppKit owns activation, Spaces and z-order.

private let floatingSpaces: NSWindow.CollectionBehavior = [
    .canJoinAllSpaces, .fullScreenAuxiliary, .ignoresCycle, .stationary,
]

/// Gives a GPUI-owned window AppKit panel semantics.
@_cdecl("subtake_configure_recorder_overlay")
public func configureRecorderOverlay(_ view: UnsafeMutableRawPointer?, _ movable: Bool) {
    guard let window = borrowedView(view)?.window else { return }
    window.styleMask = [.borderless, .nonactivatingPanel]
    window.level = .floating
    window.collectionBehavior = floatingSpaces
    window.hidesOnDeactivate = false
    window.isMovableByWindowBackground = movable
    window.animationBehavior = .utilityWindow
    window.isOpaque = false
    window.backgroundColor = .clear
    window.titleVisibility = .hidden
    window.titlebarAppearsTransparent = true
}

// MARK: - Launcher bar

private var launcherWindow: NSWindow?

@_cdecl("subtake_position_launcher")
public func positionLauncher(_ view: UnsafeMutableRawPointer?) {
    guard let window = borrowedView(view)?.window else { return }
    launcherWindow = window

    let screen = (window.screen ?? NSScreen.main)?.visibleFrame ?? .zero
    let size = window.frame.size
    window.setFrameOrigin(NSPoint(x: screen.midX - size.width / 2, y: screen.minY + 28))
}

// MARK: - Countdown

// The countdown covers the display the capture will record. It sits just under
// the bar so the bar stays pressable, and lets every click through: nothing on
// it is a control.
private var countdownWindow: NSWindow?
private var countdownDisplay: UInt32 = 0

private func placeCountdown() {
    guard let window = countdownWindow else { return }
    let key = NSDeviceDescriptionKey("NSScreenNumber")
    let target = NSScreen.screens.first { screen in
        (screen.deviceDescription[key] as? NSNumber)?.uint32Value == countdownDisplay
    } ?? launcherWindow?.screen ?? NSScreen.main
    if let target {
        window.setFrame(target.frame, display: true)
    }
}

@_cdecl("subtake_configure_countdown_overlay")
public func configureCountdownOverlay(_ view: UnsafeMutableRawPointer?) {
    guard let window = borrowedView(view)?.window else { return }
    window.styleMask = [.borderless, .nonactivatingPanel]
    window.level = NSWindow.Level(rawValue: NSWindow.Level.floating.rawValue - 1)
    window.collectionBehavior = floatingSpaces
    window.hidesOnDeactivate = false
    window.ignoresMouseEvents = true
    window.hasShadow = false
    window.isOpaque = false
    window.backgroundColor = .clear
    window.animationBehavior = .none

    countdownWindow = window
    placeCountdown()
}

@_cdecl("subtake_set_countdown_display")
public func setCountdownDisplay(_ display: UInt32) {
    countdownDisplay = display
    placeCountdown()
}

// MARK: - Options card

private var followLauncher: NSWindow?
private var followOptions: NSWindow?
private var followObserver: NSObjectProtocol?
private var optionsResizeObserver: NSObjectProtocol?

/// Where on the bar the open card belongs: the centre of the control that
/// opened it, in points from the bar's left edge. Negative centres the card on
/// the bar.
private var optionsAnchor: CGFloat = -1

private func optionsOrigin(for size: NSSize, above launcher: NSWindow) -> NSPoint {
    let bar = launcher.frame
    let screen = (launcher.screen ?? NSScreen.main)?.visibleFrame ?? .zero

    var y = bar.maxY + 14
    if y + size.height > screen.maxY {
        y = bar.minY - size.height - 14
    }
    y = max(screen.minY, min(y, screen.maxY - size.height))

    var x = bar.midX - size.width / 2
    if optionsAnchor >= 0 {
        // Over its control, but never past either end of the bar: a card
        // hanging off the bar's end reads as belonging to something else.
        x = bar.minX + optionsAnchor - size.width / 2
        x = max(bar.minX, min(x, bar.maxX - size.width))
    }
    x = max(screen.minX, min(x, screen.maxX - size.width))

    return NSPoint(x: x, y: y)
}

private func placeOptions(_ options: NSWindow?, above launcher: NSWindow?) {
    guard let options, let launcher, options.isVisible else { return }
    let current = options.frame.origin
    let origin = optionsOrigin(for: options.frame.size, above: launcher)
    if abs(current.x - origin.x) > 0.5 || abs(current.y - origin.y) > 0.5 {
        options.setFrameOrigin(origin)
    }
}

private func optionsAreFollowing(_ options: NSWindow?) -> Bool {
    guard let options else { return false }
    return options === followOptions && options.parent === followLauncher
}

@_cdecl("subtake_set_launcher_options_anchor")
public func setLauncherOptionsAnchor(_ anchor: Double) {
    optionsAnchor = anchor
    if optionsAreFollowing(followOptions) {
        placeOptions(followOptions, above: followLauncher)
    }
}

/// Resizes the card's window in one move that also puts it back above the bar.
/// Sized the usual way, AppKit keeps the top edge where it was and the card is
/// only put back a frame later. Under Reduce Motion, the one move there is,
/// that shows as the card jumping down and back.
@_cdecl("subtake_resize_launcher_options")
public func resizeLauncherOptions(_ view: UnsafeMutableRawPointer?, _ width: Double, _ height: Double) {
    // Outside the caller's GPUI update: the resize calls back into GPUI.
    DispatchQueue.main.async { [weak options = borrowedView(view)?.window] in
        guard let options else { return }
        let was = options.frame
        var frame = options.frameRect(forContentRect: NSRect(x: 0, y: 0, width: width, height: height))
        if optionsAreFollowing(options), let launcher = followLauncher {
            frame.origin = optionsOrigin(for: frame.size, above: launcher)
        } else {
            frame.origin = NSPoint(x: was.minX, y: was.maxY - frame.height)
        }
        if frame != was {
            options.setFrame(frame, display: false)
        }
    }
}

/// Fades the options card's whole window — its paint and the frosted material
/// under it together, so the two can never land apart — to `alpha` over
/// `seconds`; 0 sets it at once. Nothing here calls back into GPUI, so it runs
/// in the caller's update: a window about to be shown is already clear.
@_cdecl("subtake_fade_launcher_options")
public func fadeLauncherOptions(_ view: UnsafeMutableRawPointer?, _ alpha: Double, _ seconds: Double) {
    guard let options = borrowedView(view)?.window else { return }
    NSAnimationContext.runAnimationGroup { context in
        context.duration = seconds
        context.timingFunction = CAMediaTimingFunction(name: .easeOut)
        options.animator().alphaValue = CGFloat(alpha)
    }
    // A fade of no length lands now, over whatever fade was running.
    if seconds <= 0 {
        options.alphaValue = CGFloat(alpha)
    }
}

@_cdecl("subtake_position_launcher_options")
public func positionLauncherOptions(
    _ optionsView: UnsafeMutableRawPointer?,
    _ launcherView: UnsafeMutableRawPointer?
) {
    guard let options = borrowedView(optionsView)?.window,
          let launcher = borrowedView(launcherView)?.window
    else { return }

    // GPUI renders the options card into its own window. AppKit makes that
    // window a true child of the bar: it stays above the bar and moves with it
    // without re-rendering either GPUI tree.
    placeOptions(options, above: launcher)
    if options.parent !== launcher {
        options.parent?.removeChildWindow(options)
        launcher.addChildWindow(options, ordered: .above)
    }

    // The runtime can move its backing window directly, bypassing child-window
    // coordinate propagation. One observer is a narrow fallback for that path;
    // normal AppKit drags are handled by the child relationship.
    if followOptions !== options {
        if let optionsResizeObserver {
            NotificationCenter.default.removeObserver(optionsResizeObserver)
        }
        optionsResizeObserver = NotificationCenter.default.addObserver(
            forName: nil,
            object: options,
            queue: nil
        ) { [weak options] note in
            guard note.name == NSWindow.didResizeNotification || note.name == NSWindow.didMoveNotification
            else { return }
            // GPUI applies a resize asynchronously. Re-anchor after AppKit
            // finishes it, outside any GPUI callback's app borrow.
            DispatchQueue.main.async {
                if optionsAreFollowing(options) {
                    placeOptions(options, above: followLauncher)
                }
            }
        }
    }
    followOptions = options

    if followLauncher !== launcher {
        if let followObserver {
            NotificationCenter.default.removeObserver(followObserver)
        }
        followLauncher = launcher
        followObserver = NotificationCenter.default.addObserver(
            forName: NSWindow.didMoveNotification,
            object: launcher,
            queue: .main
        ) { _ in
            placeOptions(followOptions, above: followLauncher)
        }
    }

    options.orderFront(nil)
    // The first placement can run while the options window is still hidden.
    placeOptions(options, above: launcher)
}

@_cdecl("subtake_launcher_options_are_attached")
public func launcherOptionsAreAttached(
    _ optionsView: UnsafeMutableRawPointer?,
    _ launcherView: UnsafeMutableRawPointer?
) -> Bool {
    guard let options = borrowedView(optionsView)?.window,
          let launcher = borrowedView(launcherView)?.window,
          options.parent === launcher, options.isVisible
    else { return false }

    let current = options.frame.origin
    let expected = optionsOrigin(for: options.frame.size, above: launcher)
    return abs(current.x - expected.x) <= 2 && abs(current.y - expected.y) <= 2
}
