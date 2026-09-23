import AppKit

enum WindowCornersTests {
    static func run() {
        theWindowTakesTheRadius()
        zeroRestoresTheSystemCorners()
        nothingInsideTheWindowMasks()
    }

    static func titledWindow() -> (NSWindow, UnsafeMutableRawPointer) {
        let window = NSWindow(
            contentRect: NSRect(x: 100, y: 100, width: 1360, height: 880),
            styleMask: [.titled, .closable, .miniaturizable, .resizable, .fullSizeContentView],
            backing: .buffered,
            defer: false
        )
        return (window, Unmanaged.passUnretained(window.contentView!).toOpaque())
    }

    static func theWindowTakesTheRadius() {
        let (window, pointer) = titledWindow()
        precondition(windowCornerRadius(window) != nil, "AppKit still keeps a window corner radius")

        subtake_window_set_corner_radius(pointer, 28)
        precondition(windowCornerRadius(window) == 28, "the window rounds at the radius")

        subtake_window_set_corner_radius(pointer, 40)
        precondition(windowCornerRadius(window) == 40, "a second call changes the radius")
    }

    static func zeroRestoresTheSystemCorners() {
        let (window, pointer) = titledWindow()
        let system = windowCornerRadius(window)
        subtake_window_set_corner_radius(pointer, 28)
        subtake_window_set_corner_radius(pointer, 0)
        precondition(windowCornerRadius(window) == system, "0 hands the corners back to the system")
    }

    static func nothingInsideTheWindowMasks() {
        let (window, pointer) = titledWindow()
        let frame = window.contentView!.superview!
        let stockClip = frame.layer?.masksToBounds
        subtake_window_set_corner_radius(pointer, 28)
        // The window server rounds the window. The frame view keeps AppKit's
        // own square clip and takes no radius, so no layer rounds a second time.
        precondition(frame.layer?.masksToBounds == stockClip, "the frame view's clip is AppKit's")
        precondition(frame.layer?.cornerRadius ?? 0 == 0, "the frame view does not round")
    }
}
