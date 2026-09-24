import AppKit
import QuartzCore

/// The editor window's frosted base: one behind-window material filling the
/// window under GPUI's view. It has no mask and no radius of its own, because
/// the window's corners clip it (see WindowCorners.swift).
///
/// AppKit builds the material as a backdrop layer that samples the desktop at
/// an eighth of full resolution, which is what keeps a window-sized blur
/// cheap, and runs a blur and a saturation filter over it. Beside it sit tint
/// layers for the system's look. The tints are cleared so the theme's `bg`
/// is the only colour, and the two filters take the theme's values. AppKit
/// rebuilds these layers on appearance and focus changes, so the tuning runs
/// in `updateLayer`, after AppKit's own.
final class WindowGlass: NSVisualEffectView {
    var blurRadius: CGFloat = 0 {
        didSet { if blurRadius != oldValue { needsDisplay = true } }
    }

    var saturation: CGFloat = 1 {
        didSet { if saturation != oldValue { needsDisplay = true } }
    }

    /// The theme's `ground`: what shows where the backdrop is not drawn.
    var ground: CGColor = NSColor.black.cgColor {
        didSet { if ground != oldValue { needsDisplay = true } }
    }

    override func hitTest(_ point: NSPoint) -> NSView? {
        nil
    }

    override func updateLayer() {
        super.updateLayer()
        guard let layer else { return }
        // Mission Control and the Spaces switcher draw window snapshots
        // without backdrop layers, and the blur is rebuilt going into and
        // out of them. A base under the backdrop keeps the window reading as
        // a solid surface there; it is the theme's `ground`, not black, so
        // those frames match the frost instead of flashing dark. The live
        // blur covers it.
        layer.backgroundColor = ground
        layer.sublayers?.forEach(tune)
    }

    private func tune(_ layer: CALayer) {
        layer.backgroundColor = nil
        // The desktop tint that makes stock materials pick up the wallpaper's
        // hue. Saturation does that job here, at a value the theme chooses.
        if String(describing: type(of: layer)) == "CAChameleonLayer" {
            layer.isHidden = true
            return
        }

        if let filters = layer.filters as? [NSObject], !filters.isEmpty {
            for filter in filters {
                switch filter.value(forKey: "type") as? String {
                case "gaussianBlur": filter.setValue(blurRadius, forKey: "inputRadius")
                case "colorSaturate": filter.setValue(saturation, forKey: "inputAmount")
                default: break
                }
            }
            // A layer copies its filters, so edits land only when assigned.
            layer.filters = filters
        }
        layer.sublayers?.forEach(tune)
    }
}

private let glassKey = associationKey()

/// The window's material, if `subtake_window_set_glass` has installed one.
func windowGlass(under view: NSView) -> WindowGlass? {
    objc_getAssociatedObject(view, glassKey) as? WindowGlass
}

/// Frosts the whole window behind `view`: `blur` points of blur over the
/// desktop, at `saturation` times its colour, standing on an opaque `ground`
/// of `red`, `green`, `blue` (0–1). The first call installs the material and
/// the rest only retune it, so it is cheap to call every frame.
@_cdecl("subtake_window_set_glass")
public func subtake_window_set_glass(
    _ pointer: UnsafeMutableRawPointer?,
    _ blur: Double,
    _ saturation: Double,
    _ red: Double,
    _ green: Double,
    _ blue: Double
) {
    guard borrowedWindow(pointer) != nil, let view = borrowedView(pointer) else { return }

    var glass = windowGlass(under: view)
    if glass == nil, let parent = view.superview {
        let installed = WindowGlass(frame: view.frame)
        installed.material = .underWindowBackground
        installed.blendingMode = .behindWindow
        // Frosted whether or not the window is key. An inactive material goes
        // flat grey, and the chrome's tints are chosen to sit on the frost.
        installed.state = .active
        installed.autoresizingMask = [.width, .height]
        // Beside GPUI's view, not inside it: a child of the Metal surface
        // would be composited over the rendered UI.
        parent.addSubview(installed, positioned: .below, relativeTo: view)
        objc_setAssociatedObject(view, glassKey, installed, .OBJC_ASSOCIATION_RETAIN_NONATOMIC)
        glass = installed
    }
    glass?.blurRadius = CGFloat(blur)
    glass?.saturation = CGFloat(saturation)
    glass?.ground = CGColor(srgbRed: red, green: green, blue: blue, alpha: 1)
}
