import AppKit

/// Receives the area drawn: the display's id, then its left, top, width and
/// height in points from that display's top-left corner. A width of 0 is a
/// cancel.
public typealias AreaCallback = @convention(c) (UInt32, Double, Double, Double, Double) -> Void

/// The Source card's Area overlay: every display dimmed, and an area drawn
/// on one by dragging across it or by clicking what the pointer is over,
/// which the overlay finds as the pointer moves (`AreaSnapping`). Once
/// drawn, the area is moved by dragging it and resized by its handles,
/// every side drawn onto the edges near it unless ⌘ is held; Return or
/// Use area takes it, Esc or Cancel leaves it.
///
/// Not drawn by the design: what is offered under the pointer, how ↑ and ↓
/// widen and narrow it, the hint along the top, the edges drawn to, and
/// the arrows' nudge. The handoff has the drag alone.
final class AreaOverlay {
    static var shown: AreaOverlay?

    enum Phase {
        /// Nothing drawn: what the pointer is over is offered.
        case picking
        /// The button went down; a drag draws, a click takes the offer.
        case pressing(NSPoint)
        case drawing(anchor: NSPoint)
        /// An area is drawn and waits to be used.
        case adjusting
        case moving(grab: NSPoint, from: NSRect)
        case resizing(Handle, from: NSRect)
    }

    /// A handle, by the sides it moves.
    struct Handle: Equatable {
        var left = false
        var right = false
        var bottom = false
        var top = false
    }

    enum Button { case cancel, use }

    let palette: AreaPalette
    /// The Aspect the card holds areas to, width over height; nil for Free.
    let aspect: CGFloat?
    let callback: AreaCallback
    var windows: [AreaWindow] = []
    var phase = Phase.picking
    var selection: NSRect?
    /// The display the area is on, or the pointer is over while picking.
    var screen: NSScreen?
    var offered: [AreaCandidate] = []
    var offer = 0
    /// How far ↑ and ↓ have moved the offer from the one offered first,
    /// kept while the pointer stays over the same window.
    var widened = 0
    var over: NSRect?
    var hovered: Button?
    var guides: [AreaLine] = []
    var pointer = NSPoint.zero
    let others = AreaSnapping.windows()
    var stills: [CGDirectDisplayID: AreaStill] = [:]
    /// Each window's parts, by its frame.
    var parts: [String: AreaParts] = [:]

    init(palette: AreaPalette, aspect: CGFloat?, callback: @escaping AreaCallback) {
        self.palette = palette
        self.aspect = aspect
        self.callback = callback
    }

    // MARK: Showing

    func show(seed: (NSScreen, NSRect)?) {
        for screen in NSScreen.screens {
            let window = AreaWindow(overlay: self, screen: screen)
            windows.append(window)
        }
        if let (screen, rect) = seed {
            self.screen = screen
            selection = rect
            phase = .adjusting
        }
        // The gallery shows it without taking focus from the app in front,
        // as it shows its other windows; it takes the pointer, not keys.
        let focused = ProcessInfo.processInfo.environment["SUBTAKE_GALLERY_SCREEN"] == nil
        if focused {
            NSApp.activate(ignoringOtherApps: true)
        }
        for window in windows {
            window.alphaValue = 0
            window.orderFrontRegardless()
        }
        if focused {
            let front = windows.first { $0.screen == NSScreen.main } ?? windows.first
            front?.makeKey()
        }
        NSAnimationContext.runAnimationGroup { context in
            context.duration = AreaMetrics.fade
            for window in windows { window.animator().alphaValue = 1 }
        }
        pointer = NSEvent.mouseLocation
        refresh()
        AreaSnapping.stills { [weak self] stills in
            guard let self, Self.shown === self else { return }
            self.stills = stills
            self.pick()
            self.refresh()
        }
        pick()
        refresh()
    }

    func finish(_ area: NSRect?) {
        guard Self.shown === self else { return }
        Self.shown = nil
        let windows = windows
        NSAnimationContext.runAnimationGroup({ context in
            context.duration = AreaMetrics.fade
            for window in windows { window.animator().alphaValue = 0 }
        }, completionHandler: {
            for window in windows { window.orderOut(nil) }
        })
        NSCursor.arrow.set()
        guard let area, let screen,
              let display = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID else {
            callback(0, 0, 0, 0, 0)
            return
        }
        let frame = screen.frame
        callback(display, Double(area.minX - frame.minX), Double(frame.maxY - area.maxY), Double(area.width), Double(area.height))
    }

    func refresh() {
        for window in windows { window.overlayView.refresh() }
    }

    // MARK: Picking

    /// What the pointer is over, offered while nothing is drawn.
    func pick() {
        guard case .picking = phase else { return }
        guard let screen = NSScreen.screens.first(where: { $0.frame.contains(pointer) }) else { return }
        self.screen = screen
        let window = others.first { $0.0.contains(pointer) }
        let key = window?.0 ?? screen.frame
        if key != over {
            over = key
            widened = 0
        }
        var windowParts: AreaParts?
        if let window, let still = still(on: screen) {
            let key = NSStringFromRect(window.0)
            windowParts = parts[key] ?? AreaParts(window: window.0.intersection(screen.frame), still: still)
            parts[key] = windowParts
        }
        let (candidates, first) = AreaSnapping.candidates(at: pointer, screen: screen, window: window, parts: windowParts)
        offered = candidates.map { AreaCandidate(rect: held($0.rect, in: screen.frame), label: $0.label) }
        offer = min(max(0, first + widened), max(0, offered.count - 1))
    }

    var offering: AreaCandidate? {
        guard case .picking = phase, offer < offered.count else { return nil }
        return offered[offer]
    }

    func still(on screen: NSScreen) -> AreaStill? {
        guard let id = screen.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID else { return nil }
        return stills[id]
    }

    /// The largest rect of the card's Aspect centred in `rect`, and on the
    /// display; `rect` itself for Free.
    func held(_ rect: NSRect, in bounds: NSRect) -> NSRect {
        guard let aspect else { return rect }
        var size = rect.size
        if size.width / size.height > aspect {
            size.width = (size.height * aspect).rounded()
        } else {
            size.height = (size.width / aspect).rounded()
        }
        return NSRect(x: (rect.midX - size.width / 2).rounded(), y: (rect.midY - size.height / 2).rounded(), width: size.width, height: size.height)
            .intersection(bounds)
    }

    // MARK: Snapping

    var lines: [AreaLine] {
        guard let screen else { return [] }
        return AreaSnapping.structuralLines(windows: others.filter { $0.0.intersects(screen.frame) }, screen: screen)
            + (still(on: screen)?.lines ?? [])
    }

    /// `point` drawn onto the edges near it, the edges it went to kept to
    /// show; `point` itself while ⌘ is held.
    func snapped(_ point: NSPoint, spanning span: NSRect, snapping: Bool) -> NSPoint {
        guard snapping else { return point }
        let lines = lines
        let (x, vertical) = AreaSnapping.snap(point.x, span: span.minY...span.maxY, horizontal: false, lines: lines)
        let (y, horizontal) = AreaSnapping.snap(point.y, span: span.minX...span.maxX, horizontal: true, lines: lines)
        guides += [vertical, horizontal].compactMap { $0 }
        return NSPoint(x: x, y: y)
    }

    /// The rect from `anchor` to `corner`, kept to the ratio when there is
    /// one — the card's Aspect, or with ⇧ the one `ratio` gives — and to
    /// the display.
    func spanned(from anchor: NSPoint, to corner: NSPoint, ratio: CGFloat?, handle: Handle? = nil) -> NSRect {
        guard let bounds = screen?.frame else { return .zero }
        var corner = NSPoint(x: min(max(corner.x, bounds.minX), bounds.maxX), y: min(max(corner.y, bounds.minY), bounds.maxY))
        if let ratio, ratio > 0 {
            var width = abs(corner.x - anchor.x)
            var height = abs(corner.y - anchor.y)
            // A side's handle moves one dimension; the other follows it.
            if let handle, !(handle.left || handle.right) {
                width = height * ratio
            } else if let handle, !(handle.top || handle.bottom) {
                height = width / ratio
            } else if width / max(height, 1) > ratio {
                height = width / ratio
            } else {
                width = height * ratio
            }
            // Kept on the display, the ratio held.
            let room = NSSize(
                width: corner.x >= anchor.x ? bounds.maxX - anchor.x : anchor.x - bounds.minX,
                height: corner.y >= anchor.y ? bounds.maxY - anchor.y : anchor.y - bounds.minY
            )
            let scale = min(1, room.width / max(width, 1), room.height / max(height, 1))
            width *= scale
            height *= scale
            corner = NSPoint(
                x: anchor.x + (corner.x >= anchor.x ? width : -width),
                y: anchor.y + (corner.y >= anchor.y ? height : -height)
            )
        }
        return NSRect(x: min(anchor.x, corner.x), y: min(anchor.y, corner.y), width: abs(corner.x - anchor.x), height: abs(corner.y - anchor.y))
            .integral
    }

    func ratio(shift: Bool, current: NSRect?) -> CGFloat? {
        if let aspect { return aspect }
        guard shift else { return nil }
        guard let current, current.height > 0 else { return 1 }
        return current.width / current.height
    }

    // MARK: Events

    func mouseDown(_ event: NSEvent) {
        pointer = NSEvent.mouseLocation
        if case .adjusting = phase, let selection {
            if let button = button(at: pointer) {
                hovered = button
                refresh()
                return
            }
            if event.clickCount == 2, selection.contains(pointer) {
                finish(selection)
                return
            }
            if let handle = handle(at: pointer, of: selection) {
                phase = .resizing(handle, from: selection)
                return
            }
            if selection.contains(pointer) {
                phase = .moving(grab: pointer, from: selection)
                NSCursor.closedHand.set()
                return
            }
            // A press outside starts again.
            self.selection = nil
            screen = NSScreen.screens.first { $0.frame.contains(pointer) }
        }
        phase = .pressing(pointer)
    }

    func mouseDragged(_ event: NSEvent) {
        pointer = NSEvent.mouseLocation
        drag(event.modifierFlags)
    }

    /// The press being dragged, again: after the pointer moves, and after
    /// ⇧ or ⌘ goes down or up under a still pointer.
    func drag(_ flags: NSEvent.ModifierFlags) {
        let snapping = !flags.contains(.command)
        let shift = flags.contains(.shift)
        guides = []
        switch phase {
        case .pressing(let start):
            guard hypot(pointer.x - start.x, pointer.y - start.y) >= AreaMetrics.dragSlop else { return }
            screen = NSScreen.screens.first { $0.frame.contains(start) } ?? screen
            let anchor = snapped(start, spanning: NSRect(origin: start, size: .zero), snapping: snapping)
            phase = .drawing(anchor: anchor)
            drag(flags)
            return
        case .drawing(let anchor):
            let raw = NSRect(x: min(anchor.x, pointer.x), y: min(anchor.y, pointer.y), width: abs(pointer.x - anchor.x), height: abs(pointer.y - anchor.y))
            let corner = snapped(pointer, spanning: raw, snapping: snapping && ratio(shift: shift, current: nil) == nil)
            selection = spanned(from: anchor, to: corner, ratio: ratio(shift: shift, current: nil))
        case .moving(let grab, let from):
            guard let bounds = screen?.frame else { return }
            var rect = from.offsetBy(dx: pointer.x - grab.x, dy: pointer.y - grab.y)
            rect.origin.x = min(max(rect.minX, bounds.minX), bounds.maxX - rect.width)
            rect.origin.y = min(max(rect.minY, bounds.minY), bounds.maxY - rect.height)
            if snapping {
                let lines = lines
                let span = (x: rect.minY...rect.maxY, y: rect.minX...rect.maxX)
                let sides = [
                    (AreaSnapping.snap(rect.minX, span: span.x, horizontal: false, lines: lines), rect.minX, false),
                    (AreaSnapping.snap(rect.maxX, span: span.x, horizontal: false, lines: lines), rect.maxX, false),
                    (AreaSnapping.snap(rect.minY, span: span.y, horizontal: true, lines: lines), rect.minY, true),
                    (AreaSnapping.snap(rect.maxY, span: span.y, horizontal: true, lines: lines), rect.maxY, true),
                ]
                for horizontal in [false, true] {
                    let nearest = sides.filter { $0.2 == horizontal && $0.0.1 != nil }
                        .min { abs($0.0.0 - $0.1) < abs($1.0.0 - $1.1) }
                    guard let nearest, let line = nearest.0.1 else { continue }
                    if horizontal {
                        rect.origin.y += nearest.0.0 - nearest.1
                    } else {
                        rect.origin.x += nearest.0.0 - nearest.1
                    }
                    guides.append(line)
                }
            }
            selection = rect.integral
        case .resizing(let handle, let from):
            let anchor = NSPoint(x: handle.left ? from.maxX : from.minX, y: handle.bottom ? from.maxY : from.minY)
            var corner = NSPoint(x: handle.left ? from.minX : from.maxX, y: handle.bottom ? from.minY : from.maxY)
            if handle.left || handle.right { corner.x = pointer.x }
            if handle.top || handle.bottom { corner.y = pointer.y }
            let raw = NSRect(x: min(anchor.x, corner.x), y: min(anchor.y, corner.y), width: abs(corner.x - anchor.x), height: abs(corner.y - anchor.y))
            let ratio = ratio(shift: shift, current: from)
            if ratio == nil {
                let snappedCorner = snapped(corner, spanning: raw, snapping: snapping)
                // Only the sides the handle moves are drawn to an edge.
                if handle.left || handle.right { corner.x = snappedCorner.x }
                if handle.top || handle.bottom { corner.y = snappedCorner.y }
                guides = guides.filter { $0.horizontal ? (handle.top || handle.bottom) : (handle.left || handle.right) }
            }
            var rect = spanned(from: anchor, to: corner, ratio: ratio, handle: handle)
            // A side's handle, the ratio held, grows the other way about the
            // middle rather than from one side.
            if ratio != nil, !(handle.left || handle.right) {
                rect.origin.x = (from.midX - rect.width / 2).rounded()
            } else if ratio != nil, !(handle.top || handle.bottom) {
                rect.origin.y = (from.midY - rect.height / 2).rounded()
            }
            if let bounds = screen?.frame { rect = rect.intersection(bounds) }
            selection = rect
        default:
            return
        }
        refresh()
    }

    func mouseUp(_ event: NSEvent) {
        pointer = NSEvent.mouseLocation
        guides = []
        switch phase {
        case .pressing:
            // A click takes what is offered.
            phase = .picking
            if let offering {
                selection = offering.rect
                phase = .adjusting
            }
        case .drawing:
            if let selection, selection.width >= AreaMetrics.least, selection.height >= AreaMetrics.least {
                phase = .adjusting
            } else {
                selection = nil
                phase = .picking
                pick()
            }
        case .moving, .resizing:
            phase = .adjusting
        case .adjusting:
            if let pressed = hovered, button(at: pointer) == pressed {
                finish(pressed == .use ? selection : nil)
                return
            }
        default:
            break
        }
        updateCursor()
        refresh()
    }

    func mouseMoved() {
        pointer = NSEvent.mouseLocation
        switch phase {
        case .picking:
            pick()
        case .adjusting:
            hovered = button(at: pointer)
        default:
            break
        }
        updateCursor()
        refresh()
    }

    func scrolled(_ event: NSEvent) {
        guard case .picking = phase, event.scrollingDeltaY != 0, event.phase != .changed || abs(event.scrollingDeltaY) > 4 else { return }
        widen(event.scrollingDeltaY > 0 ? 1 : -1)
    }

    func widen(_ by: Int) {
        let next = min(max(0, offer + by), max(0, offered.count - 1))
        widened += next - offer
        offer = next
        refresh()
    }

    func keyDown(_ event: NSEvent) {
        switch event.keyCode {
        case 53:
            finish(nil)
        case 36, 76:
            if case .adjusting = phase, let selection { finish(selection) }
        case 123, 124, 125, 126:
            let step: CGFloat = event.modifierFlags.contains(.shift) ? AreaMetrics.nudgeFar : 1
            if case .picking = phase {
                if event.keyCode == 126 { widen(1) } else if event.keyCode == 125 { widen(-1) }
                return
            }
            guard case .adjusting = phase, let selection, let bounds = screen?.frame else { return }
            var rect = selection
            switch event.keyCode {
            case 123: rect.origin.x -= step
            case 124: rect.origin.x += step
            case 125: rect.origin.y -= step
            default: rect.origin.y += step
            }
            rect.origin.x = min(max(rect.minX, bounds.minX), bounds.maxX - rect.width)
            rect.origin.y = min(max(rect.minY, bounds.minY), bounds.maxY - rect.height)
            self.selection = rect
            refresh()
        default:
            break
        }
    }

    // MARK: Hit testing

    func handle(at point: NSPoint, of rect: NSRect) -> Handle? {
        let reach = AreaMetrics.handleReach
        let near = (left: abs(point.x - rect.minX) <= reach, right: abs(point.x - rect.maxX) <= reach,
                    bottom: abs(point.y - rect.minY) <= reach, top: abs(point.y - rect.maxY) <= reach)
        let within = (x: point.x >= rect.minX - reach && point.x <= rect.maxX + reach,
                      y: point.y >= rect.minY - reach && point.y <= rect.maxY + reach)
        guard within.x, within.y else { return nil }
        let handle = Handle(left: near.left, right: near.right && !near.left, bottom: near.bottom, top: near.top && !near.bottom)
        return handle == Handle() ? nil : handle
    }

    func button(at point: NSPoint) -> Button? {
        guard case .adjusting = phase else { return nil }
        let layout = self.layout
        if layout.cancel.contains(point) { return .cancel }
        if layout.use.contains(point) { return .use }
        return nil
    }

    func updateCursor() {
        switch phase {
        case .adjusting:
            guard let selection else { return }
            if button(at: pointer) != nil {
                NSCursor.pointingHand.set()
            } else if let handle = handle(at: pointer, of: selection) {
                resizeCursor(handle).set()
            } else if selection.contains(pointer) {
                NSCursor.openHand.set()
            } else {
                NSCursor.crosshair.set()
            }
        case .moving:
            NSCursor.closedHand.set()
        case .resizing(let handle, _):
            resizeCursor(handle).set()
        default:
            NSCursor.crosshair.set()
        }
    }

    private func resizeCursor(_ handle: Handle) -> NSCursor {
        if #available(macOS 15.0, *) {
            let position: NSCursor.FrameResizePosition = switch (handle.left, handle.right, handle.bottom, handle.top) {
            case (true, _, true, _): .bottomLeft
            case (true, _, _, true): .topLeft
            case (_, true, true, _): .bottomRight
            case (_, true, _, true): .topRight
            case (true, _, _, _): .left
            case (_, true, _, _): .right
            case (_, _, true, _): .bottom
            default: .top
            }
            return NSCursor.frameResize(position: position, directions: .all)
        }
        return handle.left || handle.right ? .resizeLeftRight : .resizeUpDown
    }

    // MARK: Layout

    struct Layout {
        var chip = NSRect.zero
        var row = NSRect.zero
        var cancel = NSRect.zero
        var use = NSRect.zero
        var hint = NSRect.zero
    }

    /// The rect shown: the area, or what is offered.
    var shownRect: NSRect? { selection ?? offering?.rect }

    var chipText: NSAttributedString {
        guard let rect = shownRect else { return NSAttributedString() }
        let text = NSMutableAttributedString()
        if let offering {
            text.append(NSAttributedString(string: offering.label + "  ", attributes: [.font: palette.sans(AreaMetrics.chipFont), .foregroundColor: palette.muted]))
        }
        text.append(NSAttributedString(string: "\(Int(rect.width)) × \(Int(rect.height))", attributes: [.font: palette.mono(AreaMetrics.chipFont), .foregroundColor: palette.text]))
        return text
    }

    var hintText: NSAttributedString {
        let words = switch phase {
        case .picking, .pressing:
            offered.count > 1 ? "Click to use what is highlighted, or drag to draw   ↑ ↓ wider or narrower   ⌘ draws freely" : "Drag to draw an area   ⌘ draws freely"
        default:
            "Drag the area to move it   \(aspect.map { "Held to \(Self.ratio($0))" } ?? "⇧ keeps the ratio")   ⌘ draws freely"
        }
        return NSAttributedString(string: words, attributes: [.font: palette.sans(AreaMetrics.hintFont), .foregroundColor: palette.text])
    }

    /// An aspect as the card names it, 16:9 for 1.777….
    static func ratio(_ aspect: CGFloat) -> String {
        for height in 1...AreaMetrics.ratioTerms {
            let width = (aspect * CGFloat(height)).rounded()
            if abs(width / CGFloat(height) - aspect) < AreaMetrics.ratioSlack { return "\(Int(width)):\(height)" }
        }
        return String(format: "%.2f:1", aspect)
    }

    func buttonText(_ button: Button) -> NSAttributedString {
        NSAttributedString(string: button == .use ? "Use area" : "Cancel", attributes: [
            .font: palette.sans(AreaMetrics.buttonFont),
            .foregroundColor: button == .use ? palette.onAccent : palette.text,
        ])
    }

    /// Where the chips and the confirm row sit: the size 10 under the area,
    /// the row under that; above the area when there is no room below it,
    /// and inside its foot when there is none above either.
    var layout: Layout {
        var layout = Layout()
        guard let screen else { return layout }
        let bounds = screen.frame
        let hintSize = hintText.size()
        let hintWidth = (hintSize.width + AreaMetrics.hintPadding * 2).rounded(.up)
        layout.hint = NSRect(x: (bounds.midX - hintWidth / 2).rounded(), y: screen.visibleFrame.maxY - AreaMetrics.hintInset - AreaMetrics.hintHeight,
                             width: hintWidth, height: AreaMetrics.hintHeight)
        guard let rect = shownRect else { return layout }
        let chipWidth = (chipText.size().width + AreaMetrics.chipPadding * 2).rounded(.up)
        let confirming: Bool = { if case .adjusting = phase { return true }; return false }()
        let buttons = [Button.cancel, .use].map { (buttonText($0).size().width + AreaMetrics.buttonPadding * 2).rounded(.up) }
        let rowWidth = buttons.reduce(0, +) + AreaMetrics.rowGap + AreaMetrics.rowPadding * 2
        let rowHeight = AreaMetrics.buttonHeight + AreaMetrics.rowPadding * 2
        let gap = AreaMetrics.chipGap
        let below = AreaMetrics.chipHeight + (confirming ? gap + rowHeight : 0)
        // The chip's top, then the row's, top down from the area.
        var chipTop: CGFloat
        if rect.minY - gap - below >= bounds.minY + gap {
            chipTop = rect.minY - gap
        } else if rect.maxY + gap + below <= bounds.maxY - gap {
            chipTop = rect.maxY + gap + below
        } else {
            chipTop = rect.minY + gap + below
        }
        let centre = min(max(rect.midX, bounds.minX + max(chipWidth, rowWidth) / 2 + gap), bounds.maxX - max(chipWidth, rowWidth) / 2 - gap)
        layout.chip = NSRect(x: (centre - chipWidth / 2).rounded(), y: chipTop - AreaMetrics.chipHeight, width: chipWidth, height: AreaMetrics.chipHeight)
        if confirming {
            layout.row = NSRect(x: (centre - rowWidth / 2).rounded(), y: layout.chip.minY - gap - rowHeight, width: rowWidth, height: rowHeight)
            layout.cancel = NSRect(x: layout.row.minX + AreaMetrics.rowPadding, y: layout.row.minY + AreaMetrics.rowPadding,
                                   width: buttons[0], height: AreaMetrics.buttonHeight)
            layout.use = NSRect(x: layout.cancel.maxX + AreaMetrics.rowGap, y: layout.cancel.minY, width: buttons[1], height: AreaMetrics.buttonHeight)
        }
        return layout
    }
}

/// The overlay's measures, from the handoff where it gives them.
enum AreaMetrics {
    static let fade: TimeInterval = 0.14
    /// Outside the area dimmed by `rgba(10,12,20,.48)`; the area's edge 1.5
    /// of white just outside it, 10 corner handles and 8 × 8 side handles,
    /// each casting `0 1 3` at 0.4.
    static let dim = NSColor(srgbRed: 10 / 255, green: 12 / 255, blue: 20 / 255, alpha: 0.48)
    static let outline: CGFloat = 1.5
    static let corner: CGFloat = 10
    static let side: CGFloat = 8
    static let sideRadius: CGFloat = 2
    /// The size chip 10 under the area, 24 tall in Geist Mono 500 at 11,
    /// and the confirm row 10 under that: 40 buttons, 18 in, 6 apart and
    /// 6 inside the row, in Geist 500 at 13.
    static let chipGap: CGFloat = 10
    static let chipHeight: CGFloat = 24
    static let chipPadding: CGFloat = 10
    static let chipFont: CGFloat = 11
    static let buttonHeight: CGFloat = 40
    static let buttonPadding: CGFloat = 18
    static let buttonFont: CGFloat = 13
    static let rowGap: CGFloat = 6
    static let rowPadding: CGFloat = 6
    /// The hint along the top: 32 tall, 16 in, 16 under the menu bar, in
    /// Geist 500 at 12.
    static let hintHeight: CGFloat = 32
    static let hintPadding: CGFloat = 16
    static let hintInset: CGFloat = 16
    static let hintFont: CGFloat = 12
    /// A press moves this far before it draws rather than clicks; a drawn
    /// area smaller than `least` either way is let go.
    static let dragSlop: CGFloat = 4
    static let least: CGFloat = 16
    /// How near a handle a press takes it, and how far ⇧ and an arrow
    /// nudge the area.
    static let handleReach: CGFloat = 8
    static let nudgeFar: CGFloat = 10
    /// The largest side, and how near, a ratio is named by in the hint.
    static let ratioTerms = 32
    static let ratioSlack: CGFloat = 0.005
}

/// The app's palette for the overlay's chips and buttons, sent by the
/// caller so they are the cards' own.
struct AreaPalette {
    let accent: NSColor
    let accentHover: NSColor
    let onAccent: NSColor
    let sunk: NSColor
    let sunkHover: NSColor
    let text: NSColor
    let muted: NSColor
    let frost: NSColor
    let line: NSColor
    let dark: Bool
    let sansFace: CGFont?
    let monoFace: CGFont?

    func sans(_ size: CGFloat) -> NSFont {
        sansFace.map { CTFontCreateWithGraphicsFont($0, size, nil, nil) as NSFont } ?? .systemFont(ofSize: size, weight: .medium)
    }

    func mono(_ size: CGFloat) -> NSFont {
        monoFace.map { CTFontCreateWithGraphicsFont($0, size, nil, nil) as NSFont } ?? .monospacedDigitSystemFont(ofSize: size, weight: .medium)
    }
}

/// One display's part of the overlay.
final class AreaWindow: NSWindow {
    weak var overlay: AreaOverlay?
    let overlayView: AreaView

    init(overlay: AreaOverlay, screen: NSScreen) {
        self.overlay = overlay
        overlayView = AreaView(overlay: overlay)
        super.init(contentRect: screen.frame, styleMask: [.borderless], backing: .buffered, defer: false)
        setFrame(screen.frame, display: false)
        level = .screenSaver
        collectionBehavior = [.canJoinAllSpaces, .fullScreenAuxiliary, .stationary, .ignoresCycle]
        isOpaque = false
        backgroundColor = .clear
        hasShadow = false
        isReleasedWhenClosed = false
        acceptsMouseMovedEvents = true
        // Set, not left at its default, so the parts left clear, the area
        // and what is offered, take the pointer rather than passing it
        // through to the window under them.
        ignoresMouseEvents = false
        appearance = NSAppearance(named: overlay.palette.dark ? .darkAqua : .aqua)
        contentView = overlayView
    }

    override var canBecomeKey: Bool { true }
    override var canBecomeMain: Bool { true }

    override func keyDown(with event: NSEvent) { overlay?.keyDown(event) }
    override func flagsChanged(with event: NSEvent) {
        guard let overlay else { return }
        overlay.drag(event.modifierFlags)
    }
    override func cancelOperation(_ sender: Any?) { overlay?.finish(nil) }
}

/// Draws the dim, the area and its handles, and places the frosted chips
/// and the confirm row over them.
final class AreaView: NSView {
    weak var overlay: AreaOverlay?
    let hint: FrostPlate
    let chip: FrostPlate
    let row: FrostPlate

    init(overlay: AreaOverlay) {
        self.overlay = overlay
        hint = FrostPlate(palette: overlay.palette)
        chip = FrostPlate(palette: overlay.palette)
        row = FrostPlate(palette: overlay.palette)
        super.init(frame: .zero)
        for plate in [hint, chip, row] {
            plate.isHidden = true
            addSubview(plate)
        }
        hint.content.drawing = { [weak self] bounds in
            guard let text = self?.overlay?.hintText else { return }
            let size = text.size()
            text.draw(at: NSPoint(x: (bounds.width - size.width) / 2, y: (bounds.height - size.height) / 2))
        }
        chip.content.drawing = { [weak self] bounds in
            guard let text = self?.overlay?.chipText else { return }
            let size = text.size()
            text.draw(at: NSPoint(x: (bounds.width - size.width) / 2, y: (bounds.height - size.height) / 2))
        }
        row.content.drawing = { [weak self] _ in
            guard let self, let overlay = self.overlay else { return }
            let layout = overlay.layout
            for (button, frame) in [(AreaOverlay.Button.cancel, layout.cancel), (.use, layout.use)] {
                let local = frame.offsetBy(dx: -layout.row.minX, dy: -layout.row.minY)
                let hovered = overlay.hovered == button
                let fill = button == .use ? (hovered ? overlay.palette.accentHover : overlay.palette.accent)
                    : (hovered ? overlay.palette.sunkHover : overlay.palette.sunk)
                fill.setFill()
                NSBezierPath(roundedRect: local, xRadius: local.height / 2, yRadius: local.height / 2).fill()
                let text = overlay.buttonText(button)
                let size = text.size()
                text.draw(at: NSPoint(x: local.midX - size.width / 2, y: local.midY - size.height / 2))
            }
        }
    }

    required init?(coder: NSCoder) { nil }

    override var acceptsFirstResponder: Bool { true }
    override func acceptsFirstMouse(for event: NSEvent?) -> Bool { true }

    override func updateTrackingAreas() {
        super.updateTrackingAreas()
        for area in trackingAreas { removeTrackingArea(area) }
        addTrackingArea(NSTrackingArea(rect: .zero, options: [.mouseMoved, .activeAlways, .inVisibleRect, .cursorUpdate], owner: self))
    }

    override func mouseMoved(with event: NSEvent) { overlay?.mouseMoved() }
    override func cursorUpdate(with event: NSEvent) { overlay?.updateCursor() }
    override func mouseDown(with event: NSEvent) { overlay?.mouseDown(event) }
    override func mouseDragged(with event: NSEvent) { overlay?.mouseDragged(event) }
    override func mouseUp(with event: NSEvent) { overlay?.mouseUp(event) }
    override func scrollWheel(with event: NSEvent) { overlay?.scrolled(event) }
    override func rightMouseDown(with event: NSEvent) {}

    /// Global coordinates to this view's.
    private func local(_ rect: NSRect) -> NSRect {
        guard let window else { return rect }
        return rect.offsetBy(dx: -window.frame.minX, dy: -window.frame.minY)
    }

    private var onThisScreen: Bool {
        guard let overlay, let window, let screen = overlay.screen else { return false }
        return screen.frame == window.frame
    }

    func refresh() {
        needsDisplay = true
        guard let overlay, onThisScreen else {
            for plate in [hint, chip, row] { plate.isHidden = true }
            return
        }
        let layout = overlay.layout
        hint.frame = local(layout.hint)
        hint.isHidden = layout.hint.isEmpty
        chip.frame = local(layout.chip)
        chip.isHidden = layout.chip.isEmpty
        row.frame = local(layout.row)
        row.isHidden = layout.row.isEmpty
        for plate in [hint, chip, row] {
            plate.needsLayout = true
            plate.content.needsDisplay = true
        }
    }

    override func draw(_ dirtyRect: NSRect) {
        guard let overlay else { return }
        let shown = onThisScreen ? overlay.shownRect.map(local) : nil
        let dim = NSBezierPath(rect: bounds)
        if let shown { dim.append(NSBezierPath(rect: shown)) }
        dim.windingRule = .evenOdd
        AreaMetrics.dim.setFill()
        dim.fill()
        guard let shown else { return }

        // The edges the sides went to, faint along their length.
        overlay.palette.accent.withAlphaComponent(0.8).setFill()
        for line in overlay.guides {
            let rect = line.horizontal
                ? NSRect(x: line.from, y: line.at - 0.5, width: line.to - line.from, height: 1)
                : NSRect(x: line.at - 0.5, y: line.from, width: 1, height: line.to - line.from)
            NSBezierPath(rect: local(rect)).fill()
        }

        let edge = NSBezierPath(rect: shown.insetBy(dx: -AreaMetrics.outline / 2, dy: -AreaMetrics.outline / 2))
        edge.lineWidth = AreaMetrics.outline
        NSColor.white.withAlphaComponent(overlay.selection == nil ? 0.85 : 1).setStroke()
        edge.stroke()
        guard overlay.selection != nil else { return }

        NSGraphicsContext.saveGraphicsState()
        let shadow = NSShadow()
        shadow.shadowColor = NSColor.black.withAlphaComponent(0.4)
        shadow.shadowOffset = NSSize(width: 0, height: -1)
        shadow.shadowBlurRadius = 3
        shadow.set()
        NSColor.white.setFill()
        let corner = AreaMetrics.corner
        for x in [shown.minX, shown.maxX] {
            for y in [shown.minY, shown.maxY] {
                NSBezierPath(ovalIn: NSRect(x: x - corner / 2, y: y - corner / 2, width: corner, height: corner)).fill()
            }
        }
        let side = AreaMetrics.side
        for (x, y) in [(shown.midX, shown.minY), (shown.midX, shown.maxY), (shown.minX, shown.midY), (shown.maxX, shown.midY)] {
            NSBezierPath(roundedRect: NSRect(x: x - side / 2, y: y - side / 2, width: side, height: side), xRadius: AreaMetrics.sideRadius, yRadius: AreaMetrics.sideRadius).fill()
        }
        NSGraphicsContext.restoreGraphicsState()
    }
}

/// A capsule of the recorder's frost: the desktop blurred under the
/// `frost` tint, and what it holds drawn over both. It takes no events;
/// the overlay hit-tests its buttons itself.
final class FrostPlate: NSView {
    let effect = NSVisualEffectView()
    let tint = NSView()
    let content = PlateContent()
    let palette: AreaPalette

    init(palette: AreaPalette) {
        self.palette = palette
        super.init(frame: .zero)
        // Layers throughout, or the words would draw into the window under
        // the effect's layer rather than over it.
        wantsLayer = true
        content.wantsLayer = true
        content.layerContentsRedrawPolicy = .onSetNeedsDisplay
        effect.material = .hudWindow
        effect.blendingMode = .behindWindow
        effect.state = .active
        effect.appearance = NSAppearance(named: palette.dark ? .darkAqua : .aqua)
        tint.wantsLayer = true
        tint.layer?.backgroundColor = palette.frost.cgColor
        tint.layer?.borderColor = palette.line.cgColor
        tint.layer?.borderWidth = 0.5
        for view in [effect, tint, content] { addSubview(view) }
    }

    required init?(coder: NSCoder) { nil }

    override func hitTest(_ point: NSPoint) -> NSView? { nil }

    override func layout() {
        super.layout()
        let radius = bounds.height / 2
        for view in [effect, tint, content] { view.frame = bounds }
        tint.layer?.cornerRadius = radius
        if effect.maskImage?.size.height != radius * 2 + 1 {
            let side = radius * 2 + 1
            let mask = NSImage(size: NSSize(width: side, height: side), flipped: false) { rect in
                NSColor.white.setFill()
                NSBezierPath(roundedRect: rect, xRadius: radius, yRadius: radius).fill()
                return true
            }
            mask.capInsets = NSEdgeInsets(top: radius, left: radius, bottom: radius, right: radius)
            mask.resizingMode = .stretch
            effect.maskImage = mask
        }
    }
}

final class PlateContent: NSView {
    var drawing: ((NSRect) -> Void)?
    override func draw(_ dirtyRect: NSRect) { drawing?(bounds) }
    override func hitTest(_ point: NSPoint) -> NSView? { nil }
}

private func colour(_ values: ArraySlice<Double>, _ index: Int) -> NSColor {
    let at = values.startIndex + index * 4
    return NSColor(srgbRed: values[at], green: values[at + 1], blue: values[at + 2], alpha: values[at + 3])
}

private func bytes(_ bytes: UnsafePointer<UInt8>?, _ length: Int) -> Data? {
    guard let bytes, length > 0 else { return nil }
    return Data(bytes: bytes, count: length)
}

private func face(_ data: Data?) -> CGFont? {
    guard let data, let provider = CGDataProvider(data: data as CFData) else { return nil }
    return CGFont(provider)
}

private var faces: (CGFont?, CGFont?)?

/// Opens the area overlay over every display. `colours` holds the light
/// palette's nine RGBA colours, 0–1 — accent, its hover, on accent, sunk,
/// its hover, text, muted, frost and line — then the dark palette's.
/// `appearance` is 1 for light, 2 for dark, and 0 to follow the system's.
/// `aspect` is the card's Aspect as width over height, 0 for Free. A `seed`
/// of any width opens with that area drawn, in the callback's terms. The
/// callback runs once, on the main thread. What the pointers hold is
/// copied before this returns; the overlay opens on the main thread after.
@_cdecl("subtake_draw_area")
public func drawArea(
    _ aspect: Double,
    _ appearance: Int32,
    _ colours: UnsafePointer<Double>?,
    _ sans: UnsafePointer<UInt8>?, _ sansLength: Int,
    _ mono: UnsafePointer<UInt8>?, _ monoLength: Int,
    _ seedDisplay: UInt32, _ seedX: Double, _ seedY: Double, _ seedWidth: Double, _ seedHeight: Double,
    _ callback: AreaCallback?
) {
    guard let callback, let colours else { return }
    let values = Array(UnsafeBufferPointer(start: colours, count: 9 * 4 * 2))
    let sans = bytes(sans, sansLength)
    let mono = bytes(mono, monoLength)
    DispatchQueue.main.async {
        guard AreaOverlay.shown == nil else { return }
        if faces == nil { faces = (face(sans), face(mono)) }
        let dark = appearance == 2
            || (appearance == 0 && NSApp.effectiveAppearance.bestMatch(from: [.aqua, .darkAqua]) == .darkAqua)
        let colours = dark ? values[(9 * 4)...] : values[..<(9 * 4)]
        let palette = AreaPalette(
            accent: colour(colours, 0), accentHover: colour(colours, 1), onAccent: colour(colours, 2),
            sunk: colour(colours, 3), sunkHover: colour(colours, 4), text: colour(colours, 5),
            muted: colour(colours, 6), frost: colour(colours, 7), line: colour(colours, 8),
            dark: dark, sansFace: faces?.0, monoFace: faces?.1
        )
        let overlay = AreaOverlay(palette: palette, aspect: aspect > 0 ? CGFloat(aspect) : nil, callback: callback)
        AreaOverlay.shown = overlay
        var seed: (NSScreen, NSRect)?
        if seedWidth > 0, let screen = NSScreen.screens.first(where: {
            ($0.deviceDescription[NSDeviceDescriptionKey("NSScreenNumber")] as? CGDirectDisplayID) == seedDisplay
        }) ?? NSScreen.screens.first {
            let frame = screen.frame
            seed = (screen, NSRect(x: frame.minX + seedX, y: frame.maxY - seedY - seedHeight, width: seedWidth, height: seedHeight))
        }
        overlay.show(seed: seed)
    }
}
