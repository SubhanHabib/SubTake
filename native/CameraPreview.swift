import AVFoundation

/// Receives one frame of the camera's picture: BGRA rows, mirrored, `width`
/// by `height` with `stride` bytes to a row, valid only for the call.
public typealias CameraFrameCallback = @convention(c) (UnsafePointer<UInt8>?, Int32, Int32, Int32) -> Void
/// Receives whether the camera may be used, once the system has been asked.
public typealias CameraAccessCallback = @convention(c) (Bool) -> Void

/// How wide the card's picture is drawn from: its 16:10 at twice the card's
/// width, the camera scaling to it before a frame is handed over.
private let previewWidth = 640
/// The card redraws for a frame no more often than this.
private let previewInterval: UInt64 = 33_000_000

/// Streams one camera into the Camera card, mirrored, as the camera's own
/// preview in FaceTime and Photo Booth is: the way a mirror shows you.
final class CameraPreview: NSObject, AVCaptureVideoDataOutputSampleBufferDelegate {
    let session = AVCaptureSession()
    let callback: CameraFrameCallback
    var sent: UInt64 = 0
    var mirrored = [UInt8]()

    init(_ callback: @escaping CameraFrameCallback) {
        self.callback = callback
    }

    func captureOutput(_ output: AVCaptureOutput, didOutput sampleBuffer: CMSampleBuffer, from connection: AVCaptureConnection) {
        let now = DispatchTime.now().uptimeNanoseconds
        guard now - sent >= previewInterval, let pixels = CMSampleBufferGetImageBuffer(sampleBuffer) else { return }
        sent = now
        CVPixelBufferLockBaseAddress(pixels, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(pixels, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(pixels) else { return }
        let width = CVPixelBufferGetWidth(pixels)
        let height = CVPixelBufferGetHeight(pixels)
        let stride = CVPixelBufferGetBytesPerRow(pixels)
        let row = width * 4
        if mirrored.count != row * height { mirrored = [UInt8](repeating: 0, count: row * height) }
        let source = base.assumingMemoryBound(to: UInt32.self)
        mirrored.withUnsafeMutableBytes { bytes in
            let target = bytes.bindMemory(to: UInt32.self).baseAddress!
            for y in 0..<height {
                let from = source + y * (stride / 4)
                let to = target + y * width
                for x in 0..<width { to[x] = from[width - 1 - x] }
            }
        }
        mirrored.withUnsafeBufferPointer { callback($0.baseAddress, Int32(width), Int32(height), Int32(row)) }
    }
}

/// Starts, stops and every frame run here, one at a time.
private let previewQueue = DispatchQueue(label: "com.subtake.camera-preview")
private var preview: CameraPreview?

/// Streams the camera with `device`'s unique id, or the system default for
/// an empty or null one, replacing any preview already running. Returns
/// false, and streams nothing, without camera access or a camera to open.
@_cdecl("subtake_camera_preview_start")
public func cameraPreviewStart(_ device: UnsafePointer<CChar>?, _ callback: CameraFrameCallback?) -> Bool {
    guard let callback, AVCaptureDevice.authorizationStatus(for: .video) == .authorized else { return false }
    let id = device.map { String(cString: $0) } ?? ""
    guard let camera = id.isEmpty ? AVCaptureDevice.default(for: .video) : AVCaptureDevice(uniqueID: id),
          let input = try? AVCaptureDeviceInput(device: camera)
    else { return false }
    let dimensions = CMVideoFormatDescriptionGetDimensions(camera.activeFormat.formatDescription)
    let height = dimensions.width > 0 ? previewWidth * Int(dimensions.height) / Int(dimensions.width) : previewWidth * 9 / 16
    let next = CameraPreview(callback)
    let output = AVCaptureVideoDataOutput()
    output.alwaysDiscardsLateVideoFrames = true
    output.videoSettings = [
        kCVPixelBufferPixelFormatTypeKey as String: kCVPixelFormatType_32BGRA,
        kCVPixelBufferWidthKey as String: previewWidth,
        kCVPixelBufferHeightKey as String: height & ~1,
    ]
    output.setSampleBufferDelegate(next, queue: previewQueue)
    next.session.beginConfiguration()
    guard next.session.canAddInput(input), next.session.canAddOutput(output) else { return false }
    next.session.addInput(input)
    next.session.addOutput(output)
    next.session.commitConfiguration()
    previewQueue.async {
        preview?.session.stopRunning()
        preview = next
        next.session.startRunning()
    }
    return true
}

@_cdecl("subtake_camera_preview_stop")
public func cameraPreviewStop() {
    previewQueue.async {
        preview?.session.stopRunning()
        preview = nil
    }
}

/// Asks the system for the camera, once: the card opening with the camera
/// on is the moment to, as the picture is what it is there to show.
/// `callback` receives the answer on a system thread; with an answer
/// already given it is not asked again, and receives that.
@_cdecl("subtake_camera_request_access")
public func cameraRequestAccess(_ callback: CameraAccessCallback?) {
    guard let callback else { return }
    switch AVCaptureDevice.authorizationStatus(for: .video) {
    case .notDetermined: AVCaptureDevice.requestAccess(for: .video) { callback($0) }
    case .authorized: callback(true)
    default: callback(false)
    }
}
