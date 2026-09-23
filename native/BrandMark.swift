import AppKit

/// The optical 18 pt companion to `assets/branding/menu-bar.svg`. AppKit draws
/// it as a vector template, so macOS supplies the right colour at any scale.
func menuBarImage() -> NSImage {
    let image = NSImage(size: NSSize(width: 18, height: 18), flipped: true) { _ in
        NSColor.black.set()

        let path = NSBezierPath()
        path.lineWidth = 1.4
        path.lineCapStyle = .round
        path.lineJoinStyle = .round
        path.move(to: NSPoint(x: 6.9, y: 5.8))
        path.line(to: NSPoint(x: 2.8, y: 5.8))
        path.curve(
            to: NSPoint(x: 1.8, y: 6.8),
            controlPoint1: NSPoint(x: 2.1333, y: 5.8),
            controlPoint2: NSPoint(x: 1.8, y: 6.1333)
        )
        path.line(to: NSPoint(x: 1.8, y: 11.8))
        path.curve(
            to: NSPoint(x: 2.8, y: 12.8),
            controlPoint1: NSPoint(x: 1.8, y: 12.4667),
            controlPoint2: NSPoint(x: 2.1333, y: 12.8)
        )
        path.line(to: NSPoint(x: 6.9, y: 12.8))
        path.move(to: NSPoint(x: 11.1, y: 5.8))
        path.line(to: NSPoint(x: 15.2, y: 5.8))
        path.curve(
            to: NSPoint(x: 16.2, y: 6.8),
            controlPoint1: NSPoint(x: 15.8667, y: 5.8),
            controlPoint2: NSPoint(x: 16.2, y: 6.1333)
        )
        path.line(to: NSPoint(x: 16.2, y: 11.8))
        path.curve(
            to: NSPoint(x: 15.2, y: 12.8),
            controlPoint1: NSPoint(x: 16.2, y: 12.4667),
            controlPoint2: NSPoint(x: 15.8667, y: 12.8)
        )
        path.line(to: NSPoint(x: 11.1, y: 12.8))
        path.move(to: NSPoint(x: 9, y: 3.1))
        path.line(to: NSPoint(x: 9, y: 16.1))
        path.stroke()

        NSBezierPath(
            roundedRect: NSRect(x: 7.5, y: 0.8, width: 3, height: 3),
            xRadius: 0.85,
            yRadius: 0.85
        ).fill()
        return true
    }
    image.isTemplate = true
    image.accessibilityDescription = "SubTake"
    return image
}
