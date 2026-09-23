import AppKit
import WebKit

// Run with scripts/test-native.sh. Compiled together with native/*.swift, so
// it drives the same @_cdecl entry points Rust calls.

@main
struct RecorderGlassTests {
    static func main() {
        _ = NSApplication.shared
        materialSitsUnderTheView()
        maskIsOneStretchedRoundedSquare()
        partialPlateFollowsTheCard()
        agentWorkspaceIsItsOwnNavigationDelegate()
        WindowCornersTests.run()
        WindowGlassTests.run()
        print("NATIVE_TESTS_PASSED")
    }

    static func window(width: CGFloat, height: CGFloat) -> (NSWindow, NSView, UnsafeMutableRawPointer) {
        let window = NSWindow(
            contentRect: NSRect(x: 100, y: 100, width: width, height: height),
            styleMask: .borderless,
            backing: .buffered,
            defer: false
        )
        let content = window.contentView!
        return (window, content, Unmanaged.passUnretained(content).toOpaque())
    }

    static func materialSitsUnderTheView() {
        let (window, content, pointer) = window(width: 724, height: 80)
        updateRecorderGlass(pointer, 40)

        let siblings = content.superview!.subviews
        precondition(window.contentView === content, "GPUI keeps its content view")
        precondition(siblings.count == 2, "one material beside the GPUI view")
        precondition(siblings.last === content, "GPUI stays on top")
        let glass = siblings.first as! RecorderGlass
        precondition(glass.material == .hudWindow)
        precondition(!window.hasShadow, "no window-server rim")
        precondition(glass.hitTest(NSPoint(x: 100, y: 100)) == nil, "clicks pass through")

        window.setContentSize(NSSize(width: 430, height: 264))
        updateRecorderGlass(pointer, 40)
        precondition(content.superview!.subviews.count == 2, "no duplicate material on update")
        precondition(glass.frame == content.frame, "the material fills the resized window")
    }

    static func maskIsOneStretchedRoundedSquare() {
        let (_, content, pointer) = window(width: 724, height: 80)
        updateRecorderGlass(pointer, 40)
        let glass = content.superview!.subviews.first as! RecorderGlass
        let mask = glass.maskImage!

        precondition(mask.size == NSSize(width: 81, height: 81), "drawn once at 2r + 1")
        precondition(mask.capInsets.top == 40 && mask.capInsets.left == 40, "corners keep the radius")
        precondition(mask.resizingMode == .stretch)
        precondition(alpha(of: mask, x: 40.5, y: 40.5) > 0.99, "centre is solid")
        precondition(alpha(of: mask, x: 3, y: 3) < 0.01, "corner is cut away")

        updateRecorderGlass(pointer, 40)
        precondition(glass.maskImage === mask, "the same radius does not redraw")
        updateRecorderGlass(pointer, 0)
        precondition(glass.maskImage == nil, "radius 0 removes the mask")
    }

    static func partialPlateFollowsTheCard() {
        let (_, content, pointer) = window(width: 430, height: 400)
        updateRecorderGlass(pointer, 26)
        setRecorderGlassHeight(pointer, 250)
        let glass = content.superview!.subviews.first as! RecorderGlass

        precondition(glass.frame.height == 250, "material is the card's height")
        let bottom = content.superview!.isFlipped ? glass.frame.maxY : glass.frame.minY
        let windowBottom = content.superview!.isFlipped ? content.frame.maxY : content.frame.minY
        precondition(bottom == windowBottom, "card is anchored to the bottom edge")

        setRecorderGlassHeight(pointer, 0)
        precondition(glass.frame == content.frame, "0 fills the window again")
    }

    static func agentWorkspaceIsItsOwnNavigationDelegate() {
        // The Objective-C name, because `#selector` cannot tell WebKit's two
        // decidePolicyFor overloads apart.
        let selector = NSSelectorFromString("webView:decidePolicyForNavigationAction:decisionHandler:")
        precondition(AgentWorkspace.instancesRespond(to: selector), "the policy method is reachable from WebKit")
    }

    static func alpha(of image: NSImage, x: CGFloat, y: CGFloat) -> CGFloat {
        let pixels = NSBitmapImageRep(data: image.tiffRepresentation!)!
        let column = Int(x / image.size.width * CGFloat(pixels.pixelsWide))
        let row = Int(y / image.size.height * CGFloat(pixels.pixelsHigh))
        return pixels.colorAt(x: column, y: row)!.alphaComponent
    }
}
