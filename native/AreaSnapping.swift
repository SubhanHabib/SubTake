import AppKit
import ApplicationServices
import ScreenCaptureKit

// What the area overlay snaps to: the windows on screen, the parts of the
// window under the pointer, and the long straight edges drawn on each
// display — a browser's toolbar ending where its page begins, a sidebar's
// border. Every rect and line here is in AppKit's global coordinates, in
// points, the origin at the foot of the main display.

/// A rect the pointer is over that the overlay offers whole: a click takes
/// it as the area.
struct AreaCandidate {
    var rect: NSRect
    /// What it is, as the overlay's chip names it: "Web page", "Safari".
    var label: String
}

/// A straight edge the area's sides are drawn to within `AreaSnapping.reach`.
struct AreaLine {
    /// Horizontal lines sit at a y and run along x; vertical ones the other
    /// way about.
    var horizontal: Bool
    var at: CGFloat
    var from: CGFloat
    var to: CGFloat
    /// A window's or a display's edge, rather than one found in the picture.
    /// It wins a tie.
    var structural: Bool
}

/// A still of one display at a point to the pixel, in grey, and the long
/// straight edges found in it.
struct AreaStill {
    let frame: NSRect
    let width: Int
    let height: Int
    let grey: [UInt8]
    var lines: [AreaLine] = []
}

enum AreaSnapping {
    /// How near a side must come to an edge to be drawn onto it.
    static let reach: CGFloat = 6
    /// The least a candidate may be and still be offered first; anything
    /// smaller is a control, not something to record.
    static let leastOffered = NSSize(width: 160, height: 90)
    /// A candidate is not offered twice: a second whose every side is
    /// within this of one already offered is the same one.
    static let sameWithin: CGFloat = 3
    /// How far apart two neighbouring pixels must be, out of 255, to be an
    /// edge between them, and how much of a window's width or height an
    /// edge must cross to divide it.
    static let edgeContrast = 8
    static let dividing: Double = 0.8
    /// The shortest edge found in a picture that the sides are drawn to,
    /// and the widest gap along it that still counts as one edge.
    static let leastLine = 60
    static let lineGap = 2

    // MARK: Windows

    /// The other apps' windows on screen, front to back, as `(frame, app)`.
    static func windows() -> [(NSRect, String)] {
        let own = ProcessInfo.processInfo.processIdentifier
        guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly, .excludeDesktopElements], kCGNullWindowID) as? [[String: Any]] else {
            return []
        }
        return list.compactMap { info in
            guard (info[kCGWindowLayer as String] as? Int) == 0,
                  (info[kCGWindowOwnerPID as String] as? pid_t) != own,
                  (info[kCGWindowAlpha as String] as? Double ?? 1) > 0,
                  let bounds = info[kCGWindowBounds as String] as? [String: Any],
                  let frame = CGRect(dictionaryRepresentation: bounds as CFDictionary),
                  frame.width >= 40, frame.height >= 40 else { return nil }
            return (appKit(frame), info[kCGWindowOwnerName as String] as? String ?? "Window")
        }
    }

    /// A rect from the window server's coordinates, whose origin is the top
    /// of the main display, to AppKit's.
    static func appKit(_ rect: CGRect) -> NSRect {
        let top = NSScreen.screens.first?.frame.maxY ?? 0
        return NSRect(x: rect.minX, y: top - rect.maxY, width: rect.width, height: rect.height)
    }

    /// The window server's y for an AppKit point.
    static func windowServerY(_ y: CGFloat) -> CGFloat {
        (NSScreen.screens.first?.frame.maxY ?? 0) - y
    }

    // MARK: Accessibility

    /// The element under `point` and each one holding it, up to its window,
    /// when SubTake may read other apps' interfaces. The overlay never asks
    /// for that; without it this is empty and the picture stands in.
    static func elements(at point: NSPoint) -> [AreaCandidate] {
        guard AXIsProcessTrusted() else { return [] }
        let system = AXUIElementCreateSystemWide()
        AXUIElementSetMessagingTimeout(system, 0.05)
        var hit: AXUIElement?
        guard AXUIElementCopyElementAtPosition(system, Float(point.x), Float(windowServerY(point.y)), &hit) == .success,
              var element = hit else { return [] }
        var found: [AreaCandidate] = []
        for _ in 0..<24 {
            let role = attribute(element, kAXRoleAttribute) as? String ?? ""
            if let frame = frame(of: element), frame.contains(point) {
                found.append(AreaCandidate(rect: frame, label: label(role: role, element: element)))
            }
            if role == kAXWindowRole { break }
            guard let parent = attribute(element, kAXParentAttribute) else { break }
            element = parent as! AXUIElement
        }
        return found
    }

    private static func attribute(_ element: AXUIElement, _ name: String) -> AnyObject? {
        var value: AnyObject?
        return AXUIElementCopyAttributeValue(element, name as CFString, &value) == .success ? value : nil
    }

    private static func frame(of element: AXUIElement) -> NSRect? {
        guard let position = attribute(element, kAXPositionAttribute),
              let size = attribute(element, kAXSizeAttribute) else { return nil }
        var origin = CGPoint.zero
        var extent = CGSize.zero
        guard AXValueGetValue(position as! AXValue, .cgPoint, &origin),
              AXValueGetValue(size as! AXValue, .cgSize, &extent),
              extent.width >= 1, extent.height >= 1 else { return nil }
        return appKit(CGRect(origin: origin, size: extent))
    }

    private static func label(role: String, element: AXUIElement) -> String {
        switch role {
        case "AXWebArea": return "Web page"
        case kAXWindowRole: return "Window"
        case kAXScrollAreaRole: return "Scroll area"
        case kAXSplitGroupRole: return "Split view"
        default:
            let described = attribute(element, kAXRoleDescriptionAttribute) as? String ?? ""
            return described.isEmpty ? "Element" : described.prefix(1).uppercased() + described.dropFirst()
        }
    }

    // MARK: The picture

    /// A grey still of each display, SubTake's own windows left out, at one
    /// pixel to the point. A display the system will not show SubTake gives
    /// none, and the overlay snaps to windows alone there.
    static func stills(_ done: @escaping ([CGDirectDisplayID: AreaStill]) -> Void) {
        Task.detached(priority: .userInitiated) {
            guard let content = try? await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true) else {
                await MainActor.run { done([:]) }
                return
            }
            var stills: [CGDirectDisplayID: AreaStill] = [:]
            let own = content.applications.filter { $0.processID == ProcessInfo.processInfo.processIdentifier }
            let screens = await MainActor.run {
                NSScreen.screens.compactMap { screen -> (CGDirectDisplayID, NSRect)? in
                    guard let id = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID else { return nil }
                    return (id, screen.frame)
                }
            }
            for display in content.displays {
                guard let frame = screens.first(where: { $0.0 == display.displayID })?.1 else { continue }
                let filter = SCContentFilter(display: display, excludingApplications: own, exceptingWindows: [])
                let configuration = SCStreamConfiguration()
                configuration.width = Int(frame.width)
                configuration.height = Int(frame.height)
                configuration.showsCursor = false
                guard let image = try? await SCScreenshotManager.captureImage(contentFilter: filter, configuration: configuration),
                      var still = grey(image, frame: frame) else { continue }
                still.lines = lines(in: still)
                stills[display.displayID] = still
            }
            let found = stills
            await MainActor.run { done(found) }
        }
    }

    private static func grey(_ image: CGImage, frame: NSRect) -> AreaStill? {
        let width = image.width
        let height = image.height
        var grey = [UInt8](repeating: 0, count: width * height)
        let drawn: Bool = grey.withUnsafeMutableBytes { bytes in
            guard let context = CGContext(
                data: bytes.baseAddress, width: width, height: height, bitsPerComponent: 8,
                bytesPerRow: width, space: CGColorSpaceCreateDeviceGray(), bitmapInfo: CGImageAlphaInfo.none.rawValue
            ) else { return false }
            context.draw(image, in: CGRect(x: 0, y: 0, width: width, height: height))
            return true
        }
        return drawn ? AreaStill(frame: frame, width: width, height: height, grey: grey) : nil
    }

    /// The long straight edges in a still: each row and column is walked
    /// for runs of neighbouring pixels that differ, and a run at least
    /// `leastLine` long is an edge the sides can be drawn to.
    static func lines(in still: AreaStill) -> [AreaLine] {
        var lines: [AreaLine] = []
        let (width, height) = (still.width, still.height)
        still.grey.withUnsafeBufferPointer { grey in
            func differs(_ a: Int, _ b: Int) -> Bool {
                abs(Int(grey[a]) - Int(grey[b])) >= edgeContrast
            }
            // An edge between row y - 1 and row y sits at y from the top.
            for y in 1..<height {
                var start = -1
                var last = -1
                for x in 0..<width {
                    if differs(y * width + x, (y - 1) * width + x) {
                        if start < 0 || x - last > lineGap + 1 {
                            if start >= 0, last - start + 1 >= leastLine {
                                lines.append(line(still, horizontal: true, at: y, from: start, to: last + 1))
                            }
                            start = x
                        }
                        last = x
                    }
                }
                if start >= 0, last - start + 1 >= leastLine {
                    lines.append(line(still, horizontal: true, at: y, from: start, to: last + 1))
                }
            }
            for x in 1..<width {
                var start = -1
                var last = -1
                for y in 0..<height {
                    if differs(y * width + x, y * width + x - 1) {
                        if start < 0 || y - last > lineGap + 1 {
                            if start >= 0, last - start + 1 >= leastLine {
                                lines.append(line(still, horizontal: false, at: x, from: start, to: last + 1))
                            }
                            start = y
                        }
                        last = y
                    }
                }
                if start >= 0, last - start + 1 >= leastLine {
                    lines.append(line(still, horizontal: false, at: x, from: start, to: last + 1))
                }
            }
        }
        return lines
    }

    /// A line found at pixel `at` of a still, running `from` to `to`, in
    /// global coordinates.
    private static func line(_ still: AreaStill, horizontal: Bool, at: Int, from: Int, to: Int) -> AreaLine {
        let frame = still.frame
        if horizontal {
            return AreaLine(horizontal: true, at: frame.maxY - CGFloat(at), from: frame.minX + CGFloat(from), to: frame.minX + CGFloat(to), structural: false)
        }
        return AreaLine(horizontal: false, at: frame.minX + CGFloat(at), from: frame.maxY - CGFloat(to), to: frame.maxY - CGFloat(from), structural: false)
    }

    // MARK: Offering

    /// Everything the overlay could offer under `point`, smallest first,
    /// each once, and the one it offers first: the smallest big enough to
    /// be worth recording.
    static func candidates(at point: NSPoint, screen: NSScreen, window: (NSRect, String)?, parts: AreaParts?) -> ([AreaCandidate], Int) {
        var found = elements(at: point)
        if let (frame, app) = window {
            found += parts?.candidates(at: point) ?? []
            found.append(AreaCandidate(rect: frame.intersection(screen.frame), label: app))
        }
        found.append(AreaCandidate(rect: screen.frame, label: "Display"))
        var offered: [AreaCandidate] = []
        for candidate in found {
            let rect = candidate.rect.intersection(screen.frame).integral
            guard rect.width >= 24, rect.height >= 24 else { continue }
            if let same = offered.firstIndex(where: { same($0.rect, rect) }) {
                // A window's own name beats what the picture calls it.
                if offered[same].label == "Pane" || offered[same].label == "Content" {
                    offered[same].label = candidate.label
                }
                continue
            }
            offered.append(AreaCandidate(rect: rect, label: candidate.label))
        }
        offered.sort { $0.rect.width * $0.rect.height < $1.rect.width * $1.rect.height }
        let first = offered.firstIndex {
            $0.rect.width >= leastOffered.width && $0.rect.height >= leastOffered.height
        } ?? max(0, offered.count - 1)
        return (offered, first)
    }

    private static func same(_ a: NSRect, _ b: NSRect) -> Bool {
        abs(a.minX - b.minX) <= sameWithin && abs(a.maxX - b.maxX) <= sameWithin
            && abs(a.minY - b.minY) <= sameWithin && abs(a.maxY - b.maxY) <= sameWithin
    }

    /// The edges of every window and of the display, to snap to beside
    /// those found in the picture.
    static func structuralLines(windows: [(NSRect, String)], screen: NSScreen) -> [AreaLine] {
        var lines: [AreaLine] = []
        for rect in windows.map(\.0) + [screen.frame] {
            lines.append(AreaLine(horizontal: true, at: rect.minY, from: rect.minX, to: rect.maxX, structural: true))
            lines.append(AreaLine(horizontal: true, at: rect.maxY, from: rect.minX, to: rect.maxX, structural: true))
            lines.append(AreaLine(horizontal: false, at: rect.minX, from: rect.minY, to: rect.maxY, structural: true))
            lines.append(AreaLine(horizontal: false, at: rect.maxX, from: rect.minY, to: rect.maxY, structural: true))
        }
        return lines
    }

    /// Where a side at `value`, spanning `span` the other way, is drawn to:
    /// the nearest edge within `reach` that runs alongside it, and that
    /// edge; or the side where it is.
    static func snap(_ value: CGFloat, span: ClosedRange<CGFloat>, horizontal: Bool, lines: [AreaLine]) -> (CGFloat, AreaLine?) {
        var best: AreaLine?
        var distance = reach + 0.001
        for line in lines where line.horizontal == horizontal {
            let gap = abs(line.at - value)
            guard gap <= reach, line.to >= span.lowerBound - reach, line.from <= span.upperBound + reach else { continue }
            if gap < distance || (gap == distance && line.structural && best?.structural == false) {
                best = line
                distance = gap
            }
        }
        return (best?.at ?? value, best)
    }
}

/// The parts of one window, found from the picture of it on its display.
///
/// The edges that cross all of the window divide it into bands — a
/// browser's tabs, its toolbar, its page — and those that cross all of a
/// band divide that into panes. Offered under the pointer are its pane,
/// its band, and the window below each edge above the pointer, which is
/// where a browser's page is once its toolbar is left out. The edges are
/// found once for the window, and once for each band the pointer visits.
final class AreaParts {
    private let still: AreaStill
    private let left: Int
    private let right: Int
    private let top: Int
    private let bottom: Int
    /// The window's dividing rows, top down, a line a few pixels thick as
    /// one run: the band above it ends at the run's first row, the band
    /// under it starts at its last.
    private let rows: [ClosedRange<Int>]
    private var columns: [String: [ClosedRange<Int>]] = [:]

    /// `window` as its display's still shows it, or nil for one too small
    /// to divide.
    init?(window: NSRect, still: AreaStill) {
        let frame = still.frame
        left = max(0, Int((window.minX - frame.minX).rounded()))
        right = min(still.width, Int((window.maxX - frame.minX).rounded()))
        top = max(0, Int((frame.maxY - window.maxY).rounded()))
        bottom = min(still.height, Int((frame.maxY - window.minY).rounded()))
        guard right - left >= 40, bottom - top >= 40 else { return nil }
        self.still = still
        let (left, right, top, bottom) = (left, right, top, bottom)
        rows = still.grey.withUnsafeBufferPointer { grey in
            Self.runs(in: (top + 1)..<bottom) { row in
                Self.divides(across: left..<right) { column in
                    Self.contrast(grey, row * still.width + column, (row - 1) * still.width + column)
                }
            }
        }
    }

    func candidates(at point: NSPoint) -> [AreaCandidate] {
        let frame = still.frame
        let x = Int(point.x - frame.minX)
        let y = Int(frame.maxY - point.y)
        guard (left..<right).contains(x), (top..<bottom).contains(y) else { return [] }
        let above = rows.filter { $0.upperBound <= y }.reversed().map(\.upperBound)
        let bandTop = above.first ?? top
        let bandBottom = rows.first { $0.lowerBound > y }?.lowerBound ?? bottom
        let key = "\(bandTop) \(bandBottom)"
        let dividers = columns[key] ?? still.grey.withUnsafeBufferPointer { grey in
            Self.runs(in: (left + 1)..<right) { column in
                Self.divides(across: bandTop..<bandBottom) { row in
                    Self.contrast(grey, row * still.width + column, row * still.width + column - 1)
                }
            }
        }
        columns[key] = dividers
        let paneLeft = dividers.last { $0.upperBound <= x }?.upperBound ?? left
        let paneRight = dividers.first { $0.lowerBound > x }?.lowerBound ?? right
        func rect(_ l: Int, _ t: Int, _ r: Int, _ b: Int) -> NSRect {
            NSRect(x: frame.minX + CGFloat(l), y: frame.maxY - CGFloat(b), width: CGFloat(r - l), height: CGFloat(b - t))
        }
        var parts = [
            AreaCandidate(rect: rect(paneLeft, bandTop, paneRight, bandBottom), label: "Pane"),
            AreaCandidate(rect: rect(left, bandTop, right, bandBottom), label: "Content"),
        ]
        for edge in above.prefix(6) {
            parts.append(AreaCandidate(rect: rect(left, edge, right, bandBottom), label: "Content"))
            parts.append(AreaCandidate(rect: rect(left, edge, right, bottom), label: "Content"))
        }
        return parts
    }

    private static func contrast(_ grey: UnsafeBufferPointer<UInt8>, _ a: Int, _ b: Int) -> Bool {
        abs(Int(grey[a]) - Int(grey[b])) >= AreaSnapping.edgeContrast
    }

    /// Whether enough of `span` differs from its neighbour to divide it.
    private static func divides(across span: Range<Int>, _ differs: (Int) -> Bool) -> Bool {
        var count = 0
        for index in span where differs(index) { count += 1 }
        return Double(count) >= AreaSnapping.dividing * Double(span.count)
    }

    /// The indices in `range` that `divides`, neighbours joined into runs.
    private static func runs(in range: Range<Int>, _ divides: (Int) -> Bool) -> [ClosedRange<Int>] {
        var runs: [ClosedRange<Int>] = []
        for index in range where divides(index) {
            if let last = runs.last, last.upperBound == index - 1 {
                runs[runs.count - 1] = last.lowerBound...index
            } else {
                runs.append(index...index)
            }
        }
        return runs
    }
}
