import AppKit
import QuartzCore

/// The native frosted material under a GPUI view. It never receives events;
/// GPUI stays the topmost content view.
///
/// On the recorder windows the material is masked to exactly the plate GPUI
/// paints over it, the whole window at the plate's radius, so the glass and
/// the tint read as one surface. Any other shape shows the material's grey
/// wherever the two disagree.
///
/// The mask is one small rounded square, drawn once per radius and stretched
/// through its middle to whatever size the view is. The options card changes
/// height every frame while it eases, and redrawing a window-sized mask each
/// time was slow enough to drop frames and leave the glass trailing the card.
/// Moving the view's bottom-anchored frame is cheap.
final class RecorderGlass: NSVisualEffectView {
    private var maskRadius: CGFloat = 0

    /// How much of `anchor`, from its bottom edge, the plate fills; 0 for all
    /// of it. The options window is resized a moment before its card is
    /// redrawn, and the material has to follow the card, not the window.
    var plateHeight: CGFloat = 0

    /// How far above `anchor`'s bottom edge a partial plate sits, and how
    /// opaque it is: the options card rises and fades in as a menu does,
    /// drawn by GPUI, and its material follows it frame by frame. A window
    /// fade would not do: the window server blurs what is behind a window
    /// at its own pace, not at the window's alpha, so the frost led the card
    /// in and trailed it out.
    var plateBottom: CGFloat = 0
    var plateAlpha: CGFloat = 1

    /// The frame of the GPUI view the material sits under.
    var anchor: NSRect = .zero

    var radius: CGFloat {
        get { maskRadius }
        set {
            if maskRadius == newValue, newValue <= 0 || maskImage != nil { return }
            maskRadius = newValue
            guard newValue > 0 else {
                maskImage = nil
                return
            }

            let side = newValue * 2 + 1
            let mask = NSImage(size: NSSize(width: side, height: side), flipped: false) { rect in
                NSColor.white.setFill()
                NSBezierPath(roundedRect: rect, xRadius: newValue, yRadius: newValue).fill()
                return true
            }
            mask.capInsets = NSEdgeInsets(top: newValue, left: newValue, bottom: newValue, right: newValue)
            mask.resizingMode = .stretch
            maskImage = mask
        }
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }

    func place() {
        var frame = anchor
        let partial = plateHeight > 0 && plateHeight < frame.height
        let flipped = superview?.isFlipped ?? false
        if partial {
            frame.origin.y += flipped ? frame.height - plateHeight - plateBottom : plateBottom
            frame.size.height = plateHeight
        }

        // A partial plate keeps its height and its bottom edge while the window
        // resizes around it; a full one fills the window.
        autoresizingMask = partial
            ? [.width, flipped ? .minYMargin : .maxYMargin]
            : [.width, .height]

        // Core Animation would otherwise ease each move over a quarter second
        // and hold it until the run loop comes round, while GPUI's frames go
        // straight to the screen. Either way the glass trails the card as a
        // grey ghost. The move is made at once and sent at once.
        CATransaction.begin()
        CATransaction.setDisableActions(true)
        self.frame = frame
        alphaValue = plateAlpha
        CATransaction.commit()
        CATransaction.flush()
    }

    /// A material view placed directly beneath `view`, sharing its superview.
    /// Adding it as a child of GPUI's Metal surface would make AppKit
    /// composite it over the rendered UI.
    static func install(below view: NSView, material: NSVisualEffectView.Material) -> RecorderGlass? {
        guard let parent = view.superview else { return nil }
        let glass = RecorderGlass(frame: view.frame)
        glass.material = material
        glass.blendingMode = .behindWindow
        glass.state = .active
        glass.autoresizingMask = [.width, .height]
        // Preserves GPUI's content view and responder identity.
        parent.addSubview(glass, positioned: .below, relativeTo: view)
        return glass
    }
}

private let glassKey = associationKey()
private let glassDarkKey = associationKey()
private let blurKey = associationKey()

private func glass(for view: NSView, key: UnsafeRawPointer) -> RecorderGlass? {
    objc_getAssociatedObject(view, key) as? RecorderGlass
}

/// A full-window material with no mask. Only the recorder windows call it
/// now, to turn it off; the editor's material is `WindowGlass`. Transparency
/// and GPUI's own background are set independently by the caller.
@_cdecl("subtake_window_set_blur")
public func windowSetBlur(_ pointer: UnsafeMutableRawPointer?, _ enabled: Bool) {
    assert(Thread.isMainThread, "Window material must run on the UI thread")
    guard let view = borrowedView(pointer), view.window != nil else { return }

    guard enabled else {
        glass(for: view, key: blurKey)?.removeFromSuperview()
        objc_setAssociatedObject(view, blurKey, nil, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        return
    }

    var blur = glass(for: view, key: blurKey)
    if blur == nil, let installed = RecorderGlass.install(below: view, material: .underWindowBackground) {
        objc_setAssociatedObject(view, blurKey, installed, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        blur = installed
    }
    blur?.frame = view.frame
}

/// The material follows the app's theme, not the system's: a dark card over
/// light frost reads washed out, and a light one over dark frost reads grey.
private func glassAppearance(for view: NSView) -> NSAppearance? {
    let dark = (objc_getAssociatedObject(view, glassDarkKey) as? NSNumber)?.boolValue ?? false
    return NSAppearance(named: dark ? .darkAqua : .aqua)
}

@_cdecl("subtake_set_recorder_glass_dark")
public func setRecorderGlassDark(_ pointer: UnsafeMutableRawPointer?, _ dark: Bool) {
    assert(Thread.isMainThread, "Recorder material must run on the UI thread")
    guard let view = borrowedView(pointer) else { return }
    if let was = objc_getAssociatedObject(view, glassDarkKey) as? NSNumber, was.boolValue == dark {
        return
    }

    objc_setAssociatedObject(view, glassDarkKey, NSNumber(value: dark), .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
    glass(for: view, key: glassKey)?.appearance = glassAppearance(for: view)
}

@_cdecl("subtake_update_recorder_glass")
public func updateRecorderGlass(_ pointer: UnsafeMutableRawPointer?, _ radius: Double) {
    assert(Thread.isMainThread, "Recorder material must run on the UI thread")
    guard let view = borrowedView(pointer), let window = view.window else { return }

    var plate = glass(for: view, key: glassKey)
    if plate == nil {
        // The HUD material blurs the desktop and keeps its colour, which is
        // what frosted glass is. Under-window-background blurs so hard it is a
        // flat grey sheet, and Popover is a solid white one.
        guard let installed = RecorderGlass.install(below: view, material: .hudWindow) else { return }
        installed.appearance = glassAppearance(for: view)
        objc_setAssociatedObject(view, glassKey, installed, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        window.isOpaque = false
        window.backgroundColor = .clear
        if ProcessInfo.processInfo.environment["SUBTAKE_LAUNCHER_SMOKE"] != nil {
            FileHandle.standardError.write(Data("RECORDER_NATIVE_GLASS_INSTALLED\n".utf8))
        }
        plate = installed
    }
    guard let plate else { return }

    // The window server's shadow traces the window's alpha and rings it with a
    // dark rim; on a plate that fills its window, that rim is the plate's
    // outline. The plate's own hairline is its only edge.
    window.hasShadow = false
    plate.anchor = view.frame
    plate.radius = CGFloat(radius)
    plate.place()
}

@_cdecl("subtake_set_recorder_glass_height")
public func setRecorderGlassHeight(_ pointer: UnsafeMutableRawPointer?, _ height: Double) {
    assert(Thread.isMainThread, "Recorder material must run on the UI thread")
    guard let view = borrowedView(pointer), let plate = glass(for: view, key: glassKey),
          plate.plateHeight != CGFloat(height)
    else { return }
    plate.plateHeight = CGFloat(height)
    plate.anchor = view.frame
    plate.place()
}

/// Lifts the plate `bottom` points off its window's bottom edge and sets its
/// opacity, for the options card's entrance and exit (`plateBottom`).
@_cdecl("subtake_set_recorder_glass_fade")
public func setRecorderGlassFade(_ pointer: UnsafeMutableRawPointer?, _ bottom: Double, _ alpha: Double) {
    assert(Thread.isMainThread, "Recorder material must run on the UI thread")
    guard let view = borrowedView(pointer), let plate = glass(for: view, key: glassKey),
          plate.plateBottom != CGFloat(bottom) || plate.plateAlpha != CGFloat(alpha)
    else { return }
    plate.plateBottom = CGFloat(bottom)
    plate.plateAlpha = CGFloat(alpha)
    plate.anchor = view.frame
    plate.place()
}
