// Native camera and microphone companion. All sample timing and commands share
// one serial queue; pausing removes host-clock time from both streams.
// AVFoundation sample delivery and CoreMedia retiming APIs:
// https://developer.apple.com/documentation/avfoundation/avcapturevideodataoutput
// https://developer.apple.com/documentation/coremedia/cmsamplebuffercreatecopywithnewtiming(allocator:samplebuffer:sampletimingentrycount:sampletimingarray:samplebufferout:)
import Foundation
import AVFoundation
import CoreMedia
import AppKit

func emit(_ text: String) { FileHandle.standardOutput.write(Data((text + "\n").utf8)) }
func fail(_ message: String) -> Never { fputs(message + "\n", stderr); exit(1) }
final class Companion: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate, AVCaptureAudioDataOutputSampleBufferDelegate {
    let session = AVCaptureSession()
    let queue = DispatchQueue(label: "com.subtake.companion", qos: .userInitiated)
    var videoWriter: AVAssetWriter?
    var audioWriter: AVAssetWriter?
    var videoInput: AVAssetWriterInput?
    var audioInput: AVAssetWriterInput?
    var started: CMTime?
    var pauseTime: CMTime?
    var pausedDuration = CMTime.zero
    var finishing = false
    var frames = 0
    var audioBuffers = 0
    let camera: Bool
    let microphone: Bool
    let folder: URL
    init(config: [String: Any]) throws {
        camera = config["camera"] as? Bool ?? false
        microphone = config["microphone"] as? Bool ?? false
        guard let path = config["folder"] as? String else { throw NSError(domain: "SubTake", code: 1, userInfo: [NSLocalizedDescriptionKey: "Missing companion output folder"]) }
        folder = URL(fileURLWithPath: path)
        super.init()
        session.beginConfiguration()
        session.sessionPreset = .high
        if camera {
            try authorize(.video)
            let device = (config["cameraId"] as? String).flatMap { AVCaptureDevice(uniqueID: $0) } ?? AVCaptureDevice.default(for: .video)
            guard let device else { throw error("No camera is connected") }
            let input = try AVCaptureDeviceInput(device: device)
            guard session.canAddInput(input) else { throw error("Camera input is unavailable") }
            session.addInput(input)
            let output = AVCaptureVideoDataOutput()
            output.alwaysDiscardsLateVideoFrames = true
            output.videoSettings = [kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange]
            output.setSampleBufferDelegate(self, queue: queue)
            guard session.canAddOutput(output) else { throw error("Camera output is unavailable") }
            session.addOutput(output)
            let dimensions = CMVideoFormatDescriptionGetDimensions(device.activeFormat.formatDescription)
            let width = max(2, Int(dimensions.width) / 2 * 2)
            let height = max(2, Int(dimensions.height) / 2 * 2)
            videoWriter = try AVAssetWriter(outputURL: folder.appendingPathComponent("recording.webcam.mp4"), fileType: .mp4)
            let writerInput = AVAssetWriterInput(mediaType: .video, outputSettings: [AVVideoCodecKey: AVVideoCodecType.h264, AVVideoWidthKey: width, AVVideoHeightKey: height, AVVideoCompressionPropertiesKey: [AVVideoAverageBitRateKey: 8_000_000]])
            writerInput.expectsMediaDataInRealTime = true
            guard videoWriter!.canAdd(writerInput) else { throw error("Camera encoder is unavailable") }
            videoWriter!.add(writerInput); videoInput = writerInput
        }
        if microphone {
            try authorize(.audio)
            let device = (config["microphoneId"] as? String).flatMap { AVCaptureDevice(uniqueID: $0) } ?? AVCaptureDevice.default(for: .audio)
            guard let device else { throw error("No microphone is connected") }
            let input = try AVCaptureDeviceInput(device: device)
            guard session.canAddInput(input) else { throw error("Microphone input is unavailable") }
            session.addInput(input)
            let output = AVCaptureAudioDataOutput(); output.setSampleBufferDelegate(self, queue: queue)
            guard session.canAddOutput(output) else { throw error("Microphone output is unavailable") }
            session.addOutput(output)
            audioWriter = try AVAssetWriter(outputURL: folder.appendingPathComponent("recording.mic.m4a"), fileType: .m4a)
            let writerInput = AVAssetWriterInput(mediaType: .audio, outputSettings: [AVFormatIDKey: kAudioFormatMPEG4AAC, AVSampleRateKey: 48000, AVNumberOfChannelsKey: 1, AVEncoderBitRateKey: 128000])
            writerInput.expectsMediaDataInRealTime = true
            guard audioWriter!.canAdd(writerInput) else { throw error("Microphone encoder is unavailable") }
            audioWriter!.add(writerInput); audioInput = writerInput
        }
        session.commitConfiguration()
    }
    func error(_ message: String) -> NSError { NSError(domain: "SubTake", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
    func authorize(_ type: AVMediaType) throws {
        var status = AVCaptureDevice.authorizationStatus(for: type)
        if status == .notDetermined {
            let ready = DispatchSemaphore(value: 0)
            AVCaptureDevice.requestAccess(for: type) { _ in ready.signal() }
            guard ready.wait(timeout: .now() + 60) == .success else { throw error("Permission request timed out") }
            status = AVCaptureDevice.authorizationStatus(for: type)
        }
        guard status == .authorized else { throw error("Allow SubTake \(type == .audio ? "microphone" : "camera") access in System Settings → Privacy & Security") }
    }
    func prepare() { session.startRunning(); emit("Companion ready") }
    func command(_ line: String) {
        queue.async { [self] in
            let now = CMClockGetTime(CMClockGetHostTimeClock())
            switch line {
            case "start":
                guard started == nil else { return }
                for writer in [videoWriter, audioWriter].compactMap({ $0 }) {
                    guard writer.startWriting() else { fail(writer.error?.localizedDescription ?? "Cannot start companion writer") }
                    writer.startSession(atSourceTime: .zero)
                }
                started = now; emit("Companion started")
            case "pause": if pauseTime == nil { pauseTime = now }; emit("Companion paused")
            case "resume": if let pause = pauseTime { pausedDuration = pausedDuration + (now - pause) }; pauseTime = nil; emit("Companion resumed")
            case "stop":
                guard !finishing else { return }; finishing = true
                videoInput?.markAsFinished(); audioInput?.markAsFinished()
                DispatchQueue.global().async { [self] in
                    session.stopRunning()
                    let group = DispatchGroup()
                    for writer in [videoWriter, audioWriter].compactMap({ $0 }) {
                        if writer.status == .writing { group.enter(); writer.finishWriting { group.leave() } }
                    }
                    guard group.wait(timeout: .now() + 25) == .success else { fail("Companion finalization timed out") }
                    for writer in [videoWriter, audioWriter].compactMap({ $0 }) {
                        guard writer.status == .completed else { fail(writer.error?.localizedDescription ?? "Companion recording failed") }
                    }
                    guard (!camera || frames > 0) && (!microphone || audioBuffers > 0) else { fail("The selected camera or microphone produced no samples") }
                    emit("Companion stopped"); exit(0)
                }
            default: break
            }
        }
    }
    func captureOutput(_ output: AVCaptureOutput, didOutput sample: CMSampleBuffer, from connection: AVCaptureConnection) {
        guard let start = started, pauseTime == nil, !finishing, CMSampleBufferDataIsReady(sample) else { return }
        let isVideo = output is AVCaptureVideoDataOutput
        guard let input = isVideo ? videoInput : audioInput, input.isReadyForMoreMediaData else { return }
        let offset = start + pausedDuration
        guard CMSampleBufferGetPresentationTimeStamp(sample) >= offset else { return }
        var count = 0
        guard CMSampleBufferGetSampleTimingInfoArray(sample, entryCount: 0, arrayToFill: nil, entriesNeededOut: &count) == noErr else { return }
        var timing = Array(repeating: CMSampleTimingInfo(), count: count)
        guard CMSampleBufferGetSampleTimingInfoArray(sample, entryCount: count, arrayToFill: &timing, entriesNeededOut: &count) == noErr else { return }
        for index in timing.indices {
            timing[index].presentationTimeStamp = timing[index].presentationTimeStamp - offset
            if timing[index].decodeTimeStamp.isValid { timing[index].decodeTimeStamp = timing[index].decodeTimeStamp - offset }
        }
        var adjusted: CMSampleBuffer?
        guard CMSampleBufferCreateCopyWithNewTiming(allocator: kCFAllocatorDefault, sampleBuffer: sample, sampleTimingEntryCount: count, sampleTimingArray: &timing, sampleBufferOut: &adjusted) == noErr, let adjusted else { return }
        guard input.append(adjusted) else { fail((isVideo ? videoWriter : audioWriter)?.error?.localizedDescription ?? "Companion sample write failed") }
        if isVideo { frames += 1 } else { audioBuffers += 1 }
    }
}
let _ = NSApplication.shared
let config = try JSONSerialization.jsonObject(with: Data(CommandLine.arguments[1].utf8)) as! [String: Any]
let recorder = try Companion(config: config)
DispatchQueue.global(qos: .userInitiated).async {
    recorder.prepare()
    while let line = readLine() { recorder.command(line) }
    recorder.command("stop")
}
RunLoop.main.run()
