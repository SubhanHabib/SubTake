import AppKit

/// The countdown's tick: the system's Tink, soft, kept so each second
/// restarts the one sound rather than stacking new ones.
private let tick: NSSound? = {
    let sound = NSSound(named: "Tink")
    sound?.volume = 0.5
    return sound
}()

/// Plays one tick of the recorder's countdown.
@_cdecl("subtake_countdown_tick")
public func countdownTick() {
    DispatchQueue.main.async {
        tick?.stop()
        tick?.play()
    }
}
