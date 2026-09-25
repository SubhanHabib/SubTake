import AVFoundation
import CoreGraphics

/// Whether SubTake may capture `kind` — 0 the screen, 1 the microphone, 2
/// the camera — as far as it can tell without asking. The screen has no
/// "not asked yet": until it is allowed it reads as off. A microphone or
/// camera not yet asked about reads as allowed, since recording asks.
@_cdecl("subtake_capture_access")
public func captureAccess(_ kind: Int32) -> Bool {
    switch kind {
    case 0:
        return CGPreflightScreenCaptureAccess()
    default:
        let status = AVCaptureDevice.authorizationStatus(for: kind == 1 ? .audio : .video)
        return status != .denied && status != .restricted
    }
}

/// Asks for screen capture once, which also lists SubTake under Screen &
/// System Audio Recording in System Settings. Asked again, it does nothing.
@_cdecl("subtake_request_screen_access")
public func requestScreenAccess() {
    _ = CGRequestScreenCaptureAccess()
}
