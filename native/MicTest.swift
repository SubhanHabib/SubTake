import AVFoundation

/// Receives the Test's phase: the seconds of listening left, 3 to 1, then
/// -1 while the clip plays and 0 once it is over or stopped.
public typealias MicTestCallback = @convention(c) (Int32) -> Void

/// How long the Test listens.
private let listenSeconds = 3

/// The Test in progress, all of it on `meterQueue`. Each Test takes a new
/// generation, so a second or a playback ending from one before is ignored.
private var generation = 0
private var report: MicTestCallback?
private var clip: [AVAudioPCMBuffer] = []
private var engine: AVAudioEngine?

/// Records three seconds from the microphone the card is metering, then
/// plays them back at `gain`, the Input level's amplitude. Without a meter
/// running there is nothing to listen to, and the Test is over at once.
@_cdecl("subtake_mic_test_start")
public func micTestStart(_ gain: Float, _ callback: MicTestCallback?) {
    guard let callback else { return }
    meterQueue.async {
        micTestCancel()
        guard let meter else {
            callback(0)
            return
        }
        generation += 1
        let current = generation
        report = callback
        clip = []
        // Copied as it comes: the capture takes its buffers back.
        meter.listener = { sample in
            if let buffer = pcm(sample) { clip.append(buffer) }
        }
        callback(Int32(listenSeconds))
        for second in 1...listenSeconds {
            meterQueue.asyncAfter(deadline: .now() + .seconds(second)) {
                guard current == generation else { return }
                if second < listenSeconds {
                    callback(Int32(listenSeconds - second))
                } else {
                    meter.listener = nil
                    play(gain: gain, generation: current)
                }
            }
        }
    }
}

/// Stops the Test wherever it is, listening or playing.
@_cdecl("subtake_mic_test_stop")
public func micTestStop() {
    meterQueue.async { micTestCancel() }
}

/// Ends the Test in progress, if any, and says so. On `meterQueue`.
func micTestCancel() {
    generation += 1
    meter?.listener = nil
    clip = []
    engine?.stop()
    engine = nil
    report?(0)
    report = nil
}

/// Plays the clip through the default output, each buffer after the last,
/// in the standard format so the mixer takes any microphone's.
private func play(gain: Float, generation current: Int) {
    let buffers = clip
    clip = []
    guard let format = buffers.first?.format else {
        micTestCancel()
        return
    }
    let next = AVAudioEngine()
    let player = AVAudioPlayerNode()
    next.attach(player)
    next.connect(player, to: next.mainMixerNode, format: format)
    player.volume = gain
    do {
        try next.start()
    } catch {
        micTestCancel()
        return
    }
    engine = next
    report?(-1)
    for (index, buffer) in buffers.enumerated() {
        let last = index == buffers.count - 1
        player.scheduleBuffer(buffer, completionCallbackType: .dataPlayedBack) { _ in
            guard last else { return }
            meterQueue.async {
                guard current == generation else { return }
                micTestCancel()
            }
        }
    }
    player.play()
}

/// One captured buffer as PCM in the standard format, deinterleaved float.
private func pcm(_ sample: CMSampleBuffer) -> AVAudioPCMBuffer? {
    guard let description = CMSampleBufferGetFormatDescription(sample),
          let stream = CMAudioFormatDescriptionGetStreamBasicDescription(description),
          let format = AVAudioFormat(streamDescription: stream)
    else { return nil }
    let frames = AVAudioFrameCount(CMSampleBufferGetNumSamples(sample))
    guard let captured = AVAudioPCMBuffer(pcmFormat: format, frameCapacity: frames) else { return nil }
    // The list's byte sizes follow the length, so it is set first.
    captured.frameLength = frames
    guard CMSampleBufferCopyPCMDataIntoAudioBufferList(sample, at: 0, frameCount: Int32(frames), into: captured.mutableAudioBufferList) == noErr
    else { return nil }
    guard let standard = AVAudioFormat(standardFormatWithSampleRate: format.sampleRate, channels: format.channelCount) else { return nil }
    if format == standard { return captured }
    guard let converter = AVAudioConverter(from: format, to: standard),
          let converted = AVAudioPCMBuffer(pcmFormat: standard, frameCapacity: frames),
          (try? converter.convert(to: converted, from: captured)) != nil
    else { return nil }
    return converted
}
