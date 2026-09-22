import Foundation
import AppKit
import ScreenCaptureKit
import AVFoundation

let _ = NSApplication.shared
let command = CommandLine.arguments.dropFirst().first ?? "sources"
if command == "sources" || command == "sources-passive" {
    // Opening the overlay must never request access. Only an explicit source
    // selection may enter ScreenCaptureKit before permission has been granted.
    if command == "sources-passive" && !CGPreflightScreenCaptureAccess() {
        fputs("Screen capture access is unavailable for this build. Choose a source to request access.\n", stderr)
        exit(1)
    }
    Task {
        do {
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
            var sources: [[String: Any]] = content.displays.map { d in
                ["kind": "display", "nativeId": d.displayID, "name": "Display \(d.displayID) · \(d.width) × \(d.height)", "x": d.frame.minX, "y": d.frame.minY, "width": d.frame.width, "height": d.frame.height]
            }
            sources += content.windows.filter { $0.windowLayer == 0 && $0.frame.width >= 80 && $0.frame.height >= 80 && $0.owningApplication?.processID != getpid() }.map { w in
                ["kind": "window", "nativeId": w.windowID, "name": "\(w.owningApplication?.applicationName ?? "App") — \(w.title ?? "Window")", "x": w.frame.minX, "y": w.frame.minY, "width": w.frame.width, "height": w.frame.height]
            }
            let data = try JSONSerialization.data(withJSONObject: sources, options: [.sortedKeys])
            FileHandle.standardOutput.write(data)
            exit(0)
        } catch { fputs("\(error.localizedDescription)\n", stderr); exit(1) }
    }
    RunLoop.main.run()
 } else if command == "telemetry" {
    let config = try JSONSerialization.jsonObject(with: Data(CommandLine.arguments[2].utf8)) as! [String: Any]
    var started: Double? = nil
    var pauseAt: Double? = nil
    var pausedDuration = 0.0
    var previous = [false, false, false]
    var lastClickTime = -10.0
    var lastClickLocation = CGPoint.zero
    var bounds = CGRect(x: config["x"] as? Double ?? 0, y: config["y"] as? Double ?? 0, width: config["width"] as? Double ?? 1, height: config["height"] as? Double ?? 1)
    var ticks = 0
    let timer = Timer.scheduledTimer(withTimeInterval: 1.0 / 120.0, repeats: true) { _ in
        guard let start = started, pauseAt == nil else { return }
        ticks += 1
        if ticks % 30 == 0, config["kind"] as? String == "window", let id = config["nativeId"] as? UInt32,
           let windows = CGWindowListCopyWindowInfo(.optionIncludingWindow, id) as? [[String: Any]],
           let dictionary = windows.first?[kCGWindowBounds as String] as? [String: Any],
           let frame = CGRect(dictionaryRepresentation: dictionary as CFDictionary) { bounds = frame }
        guard let location = CGEvent(source: nil)?.location else { return }
        let buttons: [CGMouseButton] = [.left, .right, .center]
        var interaction = "move"
        for i in 0..<buttons.count {
            let down = CGEventSource.buttonState(.combinedSessionState, button: buttons[i])
            if down && !previous[i] {
                interaction = i == 0 ? "click" : i == 1 ? "right-click" : "middle-click"
                if i == 0 {
                    let now=ProcessInfo.processInfo.systemUptime
                    if now-lastClickTime <= NSEvent.doubleClickInterval && hypot(location.x-lastClickLocation.x,location.y-lastClickLocation.y)<5 {interaction="double-click";lastClickTime = -10}
                    else {lastClickTime=now;lastClickLocation=location}
                }
            }
            if !down && previous[i] { interaction = "mouseup" }
            previous[i] = down
        }
        let sample: [String: Any] = ["timeMs": (ProcessInfo.processInfo.systemUptime - start - pausedDuration) * 1000,
            "cx": min(1,max(0,(location.x-bounds.minX)/max(1,bounds.width))),
            "cy": min(1,max(0,(location.y-bounds.minY)/max(1,bounds.height))), "interactionType": interaction]
        if let data = try? JSONSerialization.data(withJSONObject: sample) { FileHandle.standardOutput.write(data); FileHandle.standardOutput.write(Data([10])) }
    }
    DispatchQueue.global(qos: .utility).async {
        while let line = readLine() { DispatchQueue.main.async {
            switch line {
            case "start": started = ProcessInfo.processInfo.systemUptime
            case "pause": pauseAt = ProcessInfo.processInfo.systemUptime
            case "resume": if let pause = pauseAt { pausedDuration += ProcessInfo.processInfo.systemUptime - pause }; pauseAt = nil
            case "stop": timer.invalidate(); exit(0)
            default: break
            }
        }}
        DispatchQueue.main.async { timer.invalidate(); exit(0) }
    }
    RunLoop.main.run()
} else if command == "devices" {
    let cameras=AVCaptureDevice.devices(for: .video).map { ["id":$0.uniqueID,"name":$0.localizedName] }
    let microphones=AVCaptureDevice.devices(for: .audio).map { ["id":$0.uniqueID,"name":$0.localizedName] }
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject:["cameras":cameras,"microphones":microphones]))
} else if command == "permission-status" {
    let value: [String: Any] = ["screen": CGPreflightScreenCaptureAccess(), "microphone": AVCaptureDevice.authorizationStatus(for: .audio).rawValue, "camera": AVCaptureDevice.authorizationStatus(for: .video).rawValue]
    FileHandle.standardOutput.write(try JSONSerialization.data(withJSONObject: value))
} else { fputs("Unknown platform command\n", stderr); exit(2) }
