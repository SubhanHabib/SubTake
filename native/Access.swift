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

/// Told when a microphone or camera is plugged in or taken away.
public typealias DevicesChanged = @convention(c) () -> Void

/// The observers and the discovery session that keep device changes coming.
private var deviceWatch: (AVCaptureDevice.DiscoverySession, [NSObjectProtocol])?

/// Calls `callback` on the main thread whenever a microphone or camera is
/// connected or disconnected, from now on. Watching again replaces the
/// watch. It asks for no access: listing devices needs none.
@_cdecl("subtake_devices_watch")
public func devicesWatch(_ callback: DevicesChanged?) {
    guard let callback else { return }
    if let (_, observers) = deviceWatch {
        observers.forEach(NotificationCenter.default.removeObserver)
    }
    // A live discovery session is what has AVFoundation post the changes
    // to a process that is not capturing.
    let session = AVCaptureDevice.DiscoverySession(
        deviceTypes: [.microphone, .external, .builtInWideAngleCamera, .continuityCamera, .deskViewCamera],
        mediaType: nil,
        position: .unspecified)
    let observers = [AVCaptureDevice.wasConnectedNotification, AVCaptureDevice.wasDisconnectedNotification].map {
        NotificationCenter.default.addObserver(forName: $0, object: nil, queue: .main) { _ in callback() }
    }
    deviceWatch = (session, observers)
}
