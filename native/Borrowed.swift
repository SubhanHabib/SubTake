import AppKit

// Every entry point receives GPUI's NSView as a raw pointer that GPUI owns and
// retains. These helpers borrow it for the call without taking a reference, so
// nothing here can keep a closed window alive.

/// A C callback that receives one UTF-8 string, such as a path or an action.
public typealias StringCallback = @convention(c) (UnsafePointer<CChar>?) -> Void

/// A key for `objc_getAssociatedObject`. One allocated byte gives a stable
/// address for the life of the process, unlike `&` on a Swift global.
func associationKey() -> UnsafeRawPointer {
    UnsafeRawPointer(UnsafeMutableRawPointer.allocate(byteCount: 1, alignment: 1))
}

func borrowedView(_ pointer: UnsafeMutableRawPointer?) -> NSView? {
    guard let pointer else { return nil }
    return Unmanaged<NSView>.fromOpaque(pointer).takeUnretainedValue()
}

/// The window of a borrowed GPUI view. Every window operation is UI-thread only.
func borrowedWindow(_ pointer: UnsafeMutableRawPointer?) -> NSWindow? {
    assert(Thread.isMainThread, "Window operations must run on the UI thread")
    return borrowedView(pointer)?.window
}

func send(_ text: String, to callback: StringCallback?) {
    guard let callback else { return }
    text.withCString { callback($0) }
}
