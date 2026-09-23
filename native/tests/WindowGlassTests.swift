import AppKit

enum WindowGlassTests {
    static func run() {
        oneMaterialUnderTheView()
        theFiltersTakeTheTheme()
    }

    static func shownWindow() -> (NSWindow, NSView, UnsafeMutableRawPointer) {
        let window = NSWindow(
            contentRect: NSRect(x: 100, y: 100, width: 640, height: 400),
            styleMask: [.titled, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        // Stands in for GPUI's view inside the content view.
        let gpui = NSView(frame: window.contentView!.bounds)
        window.contentView!.addSubview(gpui)
        window.orderFrontRegardless()
        return (window, gpui, Unmanaged.passUnretained(gpui).toOpaque())
    }

    static func oneMaterialUnderTheView() {
        let (window, gpui, pointer) = shownWindow()
        subtake_window_set_glass(pointer, 56, 1.7)
        subtake_window_set_glass(pointer, 56, 1.4)

        let siblings = gpui.superview!.subviews
        precondition(siblings.count == 2, "one material, however often it is tuned")
        precondition(siblings.last === gpui, "GPUI stays on top")
        let glass = siblings.first as! WindowGlass
        precondition(glass.blendingMode == .behindWindow && glass.state == .active)
        precondition(glass.maskImage == nil && glass.layer?.masksToBounds != true, "no mask of its own")
        precondition(glass.hitTest(NSPoint(x: 10, y: 10)) == nil, "clicks pass through")
        window.orderOut(nil)
    }

    static func theFiltersTakeTheTheme() {
        let (window, gpui, pointer) = shownWindow()
        subtake_window_set_glass(pointer, 56, 1.7)
        let glass = windowGlass(under: gpui)!
        glass.displayIfNeeded()
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))

        var blur: Double?
        var saturation: Double?
        func walk(_ layer: CALayer) {
            for filter in (layer.filters as? [NSObject]) ?? [] {
                switch filter.value(forKey: "type") as? String {
                case "gaussianBlur": blur = (filter.value(forKey: "inputRadius") as? NSNumber)?.doubleValue
                case "colorSaturate": saturation = (filter.value(forKey: "inputAmount") as? NSNumber)?.doubleValue
                default: break
                }
            }
            layer.sublayers?.forEach(walk)
        }
        walk(glass.layer!)
        precondition(blur == 56, "the backdrop blurs at the theme's radius, got \(String(describing: blur))")
        precondition(saturation == 1.7, "and saturates at its amount, got \(String(describing: saturation))")
        window.orderOut(nil)
    }
}
