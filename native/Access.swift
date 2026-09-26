import AppKit
import AVFoundation
import CoreGraphics

/// What the system says of `kind` now: a round trip to its privacy
/// service, several milliseconds.
private func askAccess(_ kind: Int32) -> Bool {
    switch kind {
    case 0:
        return CGPreflightScreenCaptureAccess()
    default:
        let status = AVCaptureDevice.authorizationStatus(for: kind == 1 ? .audio : .video)
        return status != .denied && status != .restricted
    }
}

/// Each kind's last answer and when it came, the kinds being asked again,
/// and who is told when an answer moves. The recorder's surfaces read
/// access after every callback, a playhead drag's seeks among them, so a
/// read is answered from here and one older than `accessFresh` is asked
/// again off the main thread rather than holding it.
private var accessRead: [Int32: (Date, Bool)] = [:]
private var accessAsking: Set<Int32> = []
private var accessChanged: DevicesChanged?
private let accessLock = NSLock()
private let accessFresh: TimeInterval = 1

/// Whether SubTake may capture `kind` — 0 the screen, 1 the microphone, 2
/// the camera — as far as it can tell without asking. The screen has no
/// "not asked yet": until it is allowed it reads as off. A microphone or
/// camera not yet asked about reads as allowed, since recording asks.
///
/// Only the first read, and the first after `subtake_forget_access`, waits
/// on the system; later ones give the last answer, which is at most a
/// second or so behind, and `subtake_access_watch`'s callback hears when it
/// moves.
@_cdecl("subtake_capture_access")
public func captureAccess(_ kind: Int32) -> Bool {
    accessLock.lock()
    let read = accessRead[kind]
    let stale = read.map { Date().timeIntervalSince($0.0) > accessFresh } ?? true
    let refresh = read != nil && stale && !accessAsking.contains(kind)
    if refresh { accessAsking.insert(kind) }
    accessLock.unlock()
    guard let (_, allowed) = read else {
        let allowed = askAccess(kind)
        accessLock.lock()
        accessRead[kind] = (Date(), allowed)
        accessLock.unlock()
        return allowed
    }
    if refresh {
        DispatchQueue.global(qos: .utility).async {
            let now = askAccess(kind)
            accessLock.lock()
            accessRead[kind] = (Date(), now)
            accessAsking.remove(kind)
            let changed = now != allowed ? accessChanged : nil
            accessLock.unlock()
            if let changed { DispatchQueue.main.async { changed() } }
        }
    }
    return allowed
}

/// Has the next read of each kind wait on the system, as once a request
/// has just been answered.
@_cdecl("subtake_forget_access")
public func forgetAccess() {
    accessLock.lock()
    accessRead.removeAll()
    accessLock.unlock()
}

/// Calls `callback` on the main thread whenever a read of access asked
/// again comes back different, from now on. Watching again replaces it.
@_cdecl("subtake_access_watch")
public func accessWatch(_ callback: DevicesChanged?) {
    accessLock.lock()
    accessChanged = callback
    accessLock.unlock()
}

/// Asks for screen capture once, which also lists SubTake under Screen &
/// System Audio Recording in System Settings. Asked again, it does nothing.
@_cdecl("subtake_request_screen_access")
public func requestScreenAccess() {
    _ = CGRequestScreenCaptureAccess()
}

/// Told when a microphone, camera or display is plugged in or taken away.
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

/// The observer and the pending call that keep display changes coming.
private var displayWatch: (NSObjectProtocol, DispatchWorkItem?)?

/// Calls `callback` on the main thread once a display has been plugged in,
/// taken away, or has changed its size or place, from now on. A change
/// arrives as several notifications while the displays settle, so they are
/// gathered into one call a second after the last. Watching again replaces
/// the watch.
@_cdecl("subtake_displays_watch")
public func displaysWatch(_ callback: DevicesChanged?) {
    guard let callback else { return }
    if let (observer, pending) = displayWatch {
        NotificationCenter.default.removeObserver(observer)
        pending?.cancel()
    }
    let observer = NotificationCenter.default.addObserver(
        forName: NSApplication.didChangeScreenParametersNotification, object: nil, queue: .main
    ) { _ in
        displayWatch?.1?.cancel()
        let call = DispatchWorkItem { callback() }
        displayWatch?.1 = call
        DispatchQueue.main.asyncAfter(deadline: .now() + 1, execute: call)
    }
    displayWatch = (observer, nil)
}
