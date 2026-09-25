import AVFoundation

/// Receives the microphone's peak in dBFS over the last tenth of a second.
public typealias LevelCallback = @convention(c) (Float) -> Void

/// Meters one microphone for the recorder's Audio card. It only listens:
/// nothing is written, and recording opens the microphone on its own.
final class MicMeter: NSObject, AVCaptureAudioDataOutputSampleBufferDelegate {
    let session = AVCaptureSession()
    let callback: LevelCallback
    var peak = -Float.infinity
    var sent = DispatchTime.now()
    /// The Microphone card's Test, listening: every buffer as it comes.
    var listener: ((CMSampleBuffer) -> Void)?

    init(_ callback: @escaping LevelCallback) {
        self.callback = callback
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        listener?(sampleBuffer)
        for channel in connection.audioChannels {
            peak = max(peak, channel.peakHoldLevel)
        }
        // A buffer lasts ten milliseconds or so; the card redraws for a level
        // about ten times a second, which a meter reads as steady.
        let now = DispatchTime.now()
        if now.uptimeNanoseconds - sent.uptimeNanoseconds >= 90_000_000 {
            callback(peak)
            peak = -.infinity
            sent = now
        }
    }
}

/// Starts, stops and every sample buffer run here, one at a time, and the
/// Test in `MicTest.swift` with them.
let meterQueue = DispatchQueue(label: "com.subtake.mic-meter")
var meter: MicMeter?

/// Meters the microphone with `device`'s unique id, or the system default for
/// an empty or null one, replacing any meter already running. Returns false,
/// and meters nothing, without microphone access: the card never asks for it.
@_cdecl("subtake_mic_meter_start")
public func micMeterStart(_ device: UnsafePointer<CChar>?, _ callback: LevelCallback?) -> Bool {
    guard let callback, AVCaptureDevice.authorizationStatus(for: .audio) == .authorized else { return false }
    let id = device.map { String(cString: $0) } ?? ""
    guard let microphone = id.isEmpty ? AVCaptureDevice.default(for: .audio) : AVCaptureDevice(uniqueID: id),
          let input = try? AVCaptureDeviceInput(device: microphone)
    else { return false }
    let next = MicMeter(callback)
    let output = AVCaptureAudioDataOutput()
    output.setSampleBufferDelegate(next, queue: meterQueue)
    guard next.session.canAddInput(input), next.session.canAddOutput(output) else { return false }
    next.session.addInput(input)
    next.session.addOutput(output)
    meterQueue.async {
        micTestCancel()
        meter?.session.stopRunning()
        meter = next
        next.session.startRunning()
    }
    return true
}

@_cdecl("subtake_mic_meter_stop")
public func micMeterStop() {
    meterQueue.async {
        micTestCancel()
        meter?.session.stopRunning()
        meter = nil
    }
}
