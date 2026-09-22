//! Sizes, radii, type scale, fonts and layout columns.

use super::Theme;

impl Theme {
    // ---- window material -------------------------------------------------

    /// Whether translucent window chrome can be composited.
    ///
    /// SubTake renders on `zeronsh/zui`, which carries the two fixes this
    /// needs and stock gpui 0.2.2 lacks: `f596cde` (destination alpha on
    /// transparent windows — Porter-Duff OVER, not additive) and `8a8954c`
    /// (macOS blurred view on `UnderWindowBackground`, because macOS 26
    /// stopped vending `CABackdropLayer` for the `Selection` material).
    ///
    /// Every surface token above assumes this is on: they are tints, not
    /// paints. With it off the whole interface would wash out.
    pub const WINDOW_GLASS_SUPPORTED: bool = true;

    // ---- metrics ---------------------------------------------------------

    pub const DISABLED_OPACITY: f32 = 0.4;
    /// How far a control dims while held. A press is the one state that
    /// must not fade: the feedback has to land with the finger, so this is
    /// applied as an immediate style rather than through the tween store.
    pub const PRESSED_OPACITY: f32 = 0.7;

    // ---- type scale ------------------------------------------------------
    //
    // Seven sizes, and nothing between them. `FONT_BODY` is the root default
    // that every element inherits, so most elements set no size at all. The
    // three above it are title sizes and belong to `FONT_TITLE`, never to a
    // control.

    /// Section labels, and Geist Mono metadata.
    pub const FONT_SMALL: f32 = 11.0;
    /// Descriptions and metadata — the step below body, always `muted`.
    pub const FONT_SECONDARY: f32 = 12.0;
    /// Every label, row and button.
    pub const FONT_BODY: f32 = 13.0;
    pub const FONT_CONTROL: f32 = Self::FONT_BODY;
    /// Record's caption — the one control allowed to be louder than body
    /// without being a title.
    pub const FONT_ACTION: f32 = 15.0;
    /// The console's timecode, and nothing else. It is the one number the
    /// editor is read from across the room, and the handoff sets it a step
    /// above body where every other Geist Mono figure sits at 11-13. It is a
    /// mono size rather than an eighth step on the interface scale: no label,
    /// row or button may take it.
    pub const FONT_TIMECODE: f32 = 14.0;
    /// An inspector's own heading.
    pub const FONT_HEADING: f32 = 19.0;
    /// A panel or dialog title.
    pub const FONT_PANEL: f32 = 22.0;
    /// Empty-state headlines — the one size that is allowed to be loud.
    pub const FONT_DISPLAY: f32 = 40.0;

    // ---- icon scale ------------------------------------------------------
    //
    // A glyph tracks the control it sits in rather than choosing for itself,
    // so the four sizes pair with the four control heights below.

    /// A caret or a disclosure arrow — the smallest glyph the set uses. It
    /// is not sized by its control: a caret is a hint about the control
    /// rather than the control's own mark, so it stays small inside a 40px
    /// trigger where a 16px glyph would read as a second icon.
    pub const ICON_SIZE_CARET: f32 = 12.0;
    /// In a 34px control: a resize grip, a menu item's tick, a checkbox.
    pub const ICON_SIZE_SMALL: f32 = 14.0;
    /// In a 40px control — the default a glyph takes.
    pub const ICON_SIZE: f32 = 16.0;
    /// In a 44px control: a primary button, the titlebar.
    pub const ICON_SIZE_MEDIUM: f32 = 17.0;
    /// In a 44–52px control: the tool pod, where a glyph is the whole
    /// control and carries no caption to share the space with.
    pub const ICON_SIZE_POD: f32 = 18.0;
    /// In a 52–60px control: Record, a hero button, the brand mark.
    pub const ICON_SIZE_LARGE: f32 = 19.0;
    /// The transport's play and pause. The one glyph drawn at fill weight,
    /// and the one drawn larger than its control's own step: it is the only
    /// mark on a 56px plate, so it is sized to the plate rather than to the
    /// ladder.
    pub const ICON_SIZE_TRANSPORT: f32 = 21.0;

    // ---- radius ----------------------------------------------------------
    //
    // The ladder, smallest first. Nothing lands between 14 and 20: that gap
    // is what keeps a container and its contents readable as separate
    // layers. Anything labelled — button, chip, segment, toggle — is a full
    // pill and takes no radius from here, and every icon button is a circle.

    /// A timeline region, a tooltip.
    pub const RADIUS_REGION: f32 = 10.0;
    /// A lane, a small thumbnail, a menu item.
    pub const RADIUS_LANE: f32 = 12.0;
    /// The screen inside the preview, a large thumbnail.
    pub const RADIUS_INNER: f32 = 14.0;
    /// A preset tile, a menu's container.
    pub const RADIUS_MENU: f32 = 20.0;
    /// A list row, a recent card.
    pub const RADIUS_ROW: f32 = 22.0;
    /// The preview frame, the player bar.
    pub const RADIUS_FRAME: f32 = 24.0;
    /// A list's plate — the recess its rows sit in.
    pub const RADIUS_PLATE: f32 = 26.0;
    /// The window shell, the console, the inspector, a dialog.
    pub const RADIUS_PANEL: f32 = 28.0;
    /// The tool pod.
    pub const RADIUS_POD: f32 = 30.0;
    /// The recorder bar.
    pub const RADIUS_BAR: f32 = 40.0;

    // ---- control heights -------------------------------------------------
    //
    // Four for buttons and three for the fixed shapes. A control's height
    // picks its glyph size and its padding; nothing sets those separately.

    /// An icon button in a dense row, a menu item, an aspect pill.
    pub const CONTROL_HEIGHT_SMALL: f32 = 34.0;
    /// A raised control — one sitting directly on glass.
    pub const CONTROL_HEIGHT: f32 = 40.0;
    /// A primary or secondary button, and the round buttons beside them.
    pub const CONTROL_HEIGHT_LARGE: f32 = 44.0;
    /// A hero button: the one action an empty screen is asking for.
    pub const CONTROL_HEIGHT_HERO: f32 = 52.0;
    /// The transport button — the largest round control.
    pub const TRANSPORT_SIZE: f32 = 56.0;
    /// Record. Taller than anything else because it is the one control that
    /// must never be hit by accident, and its dot, which pulses on a one
    /// second cycle while capture is running.
    pub const RECORD_HEIGHT: f32 = 60.0;
    pub const RECORD_DOT: f32 = 12.0;

    /// The recorder bar. The floating window IS the bar — there is no plate
    /// around a plate — so these are the window's own figures, and the bar
    /// is one row of `RECORD_HEIGHT` controls with its padding either side.
    pub const RECORDER_HEIGHT: f32 = Self::RECORD_HEIGHT + Self::RECORDER_PADDING * 2.0;
    pub const RECORDER_PADDING: f32 = 10.0;
    /// The grip at the bar's left. Narrower than a control, because it is a
    /// texture to take hold of rather than a target to hit.
    pub const RECORDER_HANDLE: f32 = 28.0;
    /// The unified titlebar, and what it keeps clear at each end: the traffic
    /// lights sit at {14,15} and the first control clears them, while the
    /// right end is a hair tighter than a float's inset because the buttons
    /// there are already round.
    pub const TITLEBAR_HEIGHT: f32 = 64.0;
    pub const TITLEBAR_TRAFFIC_LIGHTS: f32 = 92.0;
    pub const TITLEBAR_PADDING: f32 = 20.0;
    /// The document pill in the middle of the titlebar. Shorter than a
    /// control, because it is a label on a plate rather than something to
    /// press.
    pub const TITLE_PILL_HEIGHT: f32 = 36.0;

    /// A chip: a context label, a piece of metadata on a plate.
    pub const CHIP_HEIGHT: f32 = 26.0;
    pub const TOOLTIP_HEIGHT: f32 = 28.0;
    pub const FOOTER_HEIGHT: f32 = 28.0;

    // ---- horizontal rhythm -----------------------------------------------

    pub const GAP_SMALL: f32 = 4.0;
    pub const GAP: f32 = 8.0;
    pub const GAP_LARGE: f32 = 12.0;
    /// Between blocks inside a panel — one step above the control gap.
    pub const GAP_BLOCK: f32 = 14.0;
    /// A panel's own padding: the console and the inspector.
    pub const PANEL_PADDING: f32 = 18.0;
    /// A pod's padding. A pod is a holder for 40px controls, not a container
    /// with a layout of its own, so it gives them 8 and nothing more.
    pub const POD_PADDING: f32 = 8.0;
    /// A float's inset from the window edge.
    pub const INSET: f32 = 24.0;
    /// A float hanging from the top of the stage sits a step tighter than one
    /// at its side: the titlebar above it is already air.
    pub const INSET_TOP: f32 = 20.0;

    /// Side padding by control height. A labelled control is a pill, so its
    /// padding is what gives it its width.
    pub const CONTROL_PADDING_SMALL: f32 = 12.0;
    pub const CONTROL_PADDING: f32 = 16.0;
    pub const CONTROL_PADDING_LARGE: f32 = 18.0;
    pub const CONTROL_PADDING_PRIMARY: f32 = 22.0;
    pub const CONTROL_PADDING_HERO: f32 = 26.0;
    /// A text field's own sides. A hair under the button padding: a field
    /// is read along its whole width, so its text starts closer to the edge
    /// than a centred caption does.
    pub const INPUT_PADDING: f32 = 15.0;
    /// Between a glyph and the label beside it: on a button, in a row, in a
    /// pod. Three values because the gap has to grow with the control.
    pub const ICON_GAP_POD: f32 = 8.0;
    pub const ICON_GAP: f32 = 9.0;
    pub const ICON_GAP_ROW: f32 = 10.0;
    /// Record's dot to its caption. One step wider than any other button,
    /// because the dot is a state indicator rather than an icon and has to
    /// read as separate from the word beside it.
    pub const ICON_GAP_RECORD: f32 = 11.0;

    // ---- edges -----------------------------------------------------------
    //
    // There are no real borders in the redesign: every edge is an inset
    // shadow, so an edge can never change what a control measures.

    pub const BORDER_WIDTH: f32 = 1.0;
    /// A selected row, region or tile, and a focused unfilled control.
    pub const SELECTED_WIDTH: f32 = 1.5;
    /// The focus ring's spread. Keyboard only.
    pub const FOCUS_WIDTH: f32 = 3.0;
    pub const SLIDER_FOCUS_WIDTH: f32 = 1.0;
    /// The window shell's hairline.
    pub const HAIRLINE_WIDTH: f32 = 0.5;

    // ---- fixed shapes ----------------------------------------------------

    /// The toggle: a 46-wide track with a 22 thumb inset 3, so the thumb
    /// travels 18. Held, the thumb stretches to `TOGGLE_THUMB_HELD`.
    pub const TOGGLE_WIDTH: f32 = 46.0;
    pub const TOGGLE_HEIGHT: f32 = 28.0;
    pub const TOGGLE_INSET: f32 = 3.0;
    pub const TOGGLE_THUMB: f32 = Self::TOGGLE_HEIGHT - Self::TOGGLE_INSET * 2.0;
    pub const TOGGLE_THUMB_HELD: f32 = 26.0;
    pub const TOGGLE_TRAVEL: f32 =
        Self::TOGGLE_WIDTH - Self::TOGGLE_INSET * 2.0 - Self::TOGGLE_THUMB;

    /// The lane stack: the source lane is taller than the rest.
    pub const LANE_HEIGHT: f32 = 30.0;
    pub const LANE_SOURCE_HEIGHT: f32 = 42.0;
    pub const LANE_GAP: f32 = 4.0;
    /// The column of lane names, and its gap to the tracks.
    pub const LANE_GUTTER: f32 = 78.0;
    pub const LANE_GUTTER_GAP: f32 = 14.0;
    /// One lane's share of the stack: the lane plus the air under it, which
    /// is the step from one lane's top to the next one's.
    pub const LANE_PITCH: f32 = Self::LANE_HEIGHT + Self::LANE_GAP;

    // ---- timeline regions ------------------------------------------------
    //
    // A region is painted in its lane's tint at four strengths, which the
    // handoff gives as hex suffixes on that tint: `22` at rest, `2e` under
    // the pointer, `3d` when selected, `66` for the edge and `cc` for a trim
    // handle. They are alphas of one colour rather than five colours, which
    // is what keeps seven lane tints from becoming thirty-five.
    pub const REGION_FILL: f32 = 0x22 as f32 / 255.0;
    pub const REGION_FILL_HOVER: f32 = 0x2e as f32 / 255.0;
    pub const REGION_FILL_SELECTED: f32 = 0x3d as f32 / 255.0;
    pub const REGION_EDGE: f32 = 0x66 as f32 / 255.0;
    pub const REGION_HANDLE_ALPHA: f32 = 0xcc as f32 / 255.0;
    pub const REGION_PADDING: f32 = 12.0;
    /// A trim handle: a bar 3 wide, inset 3 from the region's end, running
    /// the lane's height less 6 of air at each end.
    pub const REGION_HANDLE_WIDTH: f32 = 3.0;
    pub const REGION_HANDLE_RADIUS: f32 = 2.0;
    pub const REGION_HANDLE_INSET: f32 = 3.0;
    pub const REGION_HANDLE_MARGIN: f32 = 6.0;
    /// The grab area around a handle — wider than the mark, because a 3px
    /// target is not a target.
    pub const REGION_HANDLE_TARGET: f32 = 10.0;

    /// The playhead: a rule the full height of the lane stack with a dot at
    /// its head, and a soft ring around the dot so it reads on a lane painted
    /// in its own tint.
    pub const PLAYHEAD_WIDTH: f32 = 2.0;
    pub const PLAYHEAD_DOT: f32 = 16.0;
    pub const PLAYHEAD_RING: f32 = 4.0;

    /// The filled slider's fill is inset by a hair so the plate's radius still
    /// reads at the edges.
    pub const SLIDER_FILL_INSET: f32 = 1.0;

    /// Value input inside a scrub field.
    pub const SCRUB_VALUE_WIDTH: f32 = 70.0;
    /// The tool pod: one 44px round button with the pod's own padding either
    /// side of it, and nothing else. The pod is a holder for the buttons, so
    /// its width is theirs plus its padding rather than a figure of its own.
    pub const POD_WIDTH: f32 = Self::CONTROL_HEIGHT_LARGE + Self::POD_PADDING * 2.0;
    /// A transient menu's list box: how tall it grows before it scrolls, and
    /// the width it will not shrink below when its trigger is narrower than
    /// its rows. Shared by the dropdown and the command palette so one is
    /// never a different shape from the other.
    ///
    /// The handoff says a menu has no maximum height. That is true of a menu
    /// on a 1600 x 1000 shell and false of one in the recorder's options
    /// window, so this is the ceiling a menu takes when the window it opens
    /// in is bigger than it; the dropdown lowers it to what its own window
    /// can actually show.
    pub const MENU_MAX_HEIGHT: f32 = 280.0;
    pub const MENU_MIN_WIDTH: f32 = 216.0;

    // ---- menu interior ---------------------------------------------------
    //
    // Two sections of the handoff give these different numbers: the primitive
    // reference has a 34px item at radius 12 with 12px sides on a 180-wide
    // surface, and "Menus and overlays" has a 32px item at radius 14 with
    // 10px sides on a 216-wide one. The later, dedicated section wins, and
    // the difference between hover and checked goes with it: both are `sunk`,
    // and the tick is what says which item is the current one.
    /// The surface's own padding, inside its radius.
    pub const MENU_PADDING: f32 = 6.0;
    pub const MENU_ITEM_HEIGHT: f32 = 32.0;
    pub const MENU_ITEM_PADDING: f32 = 10.0;
    /// Items sit a hair apart rather than flush, so two adjacent fills read
    /// as two rows instead of one block.
    pub const MENU_ITEM_GAP: f32 = 1.0;
    pub const MENU_ITEM_RADIUS: f32 = Self::RADIUS_INNER;
    /// A fixed column for the tick, held whether or not an item carries one,
    /// so the labels in a menu line up with each other.
    pub const CHECK_GUTTER: f32 = Self::ICON_SIZE_SMALL;
    /// A rule between groups of items: inset from both edges, with air of its
    /// own above and below.
    pub const MENU_SEPARATOR_INSET: f32 = 10.0;
    pub const MENU_SEPARATOR_MARGIN: f32 = 5.0;

    /// Small marks: the unsaved dot, a progress rule, a colour sample and a
    /// picker thumbnail. Named here so no view invents its own.
    pub const DOT_SIZE: f32 = 7.0;
    /// A progress rule: a `sunk` track under an `accent` fill, with its
    /// percentage set beside the bar rather than inside it — the bar is too
    /// thin to hold text and a number on top of a moving fill is unreadable.
    pub const PROGRESS_HEIGHT: f32 = 6.0;
    pub const PROGRESS_RADIUS: f32 = Self::PROGRESS_HEIGHT / 2.0;
    pub const SWATCH_SIZE: f32 = 26.0;
    pub const TILE_WIDTH: f32 = 68.0;
    pub const TILE_HEIGHT: f32 = 48.0;

    // ---- dialogs ----------------------------------------------------------

    /// A centred dialog — the Presets panel is the one the handoff draws.
    pub const DIALOG_WIDTH: f32 = 470.0;
    pub const DIALOG_PADDING: f32 = 22.0;
    /// A dialog's footer buttons: a step between Primary and Hero, because
    /// they end the dialog rather than act inside it.
    pub const CONTROL_HEIGHT_DIALOG: f32 = 48.0;
    /// A preset row's preview: the preset's background with its frame drawn
    /// inside at `PRESET_PREVIEW_SCALE` of its real padding and radius.
    pub const PRESET_PREVIEW_WIDTH: f32 = 74.0;
    pub const PRESET_PREVIEW_HEIGHT: f32 = 50.0;
    pub const PRESET_PREVIEW_SCALE: f32 = 0.35;

    // ---- empty state ------------------------------------------------------

    /// The empty state's column rhythm — looser than any panel's, because
    /// there is nothing else on the screen to hold it together.
    pub const EMPTY_GAP: f32 = 26.0;
    /// The empty state's side and bottom margins, so the column centres on
    /// the stage a little above the window's middle.
    pub const EMPTY_PADDING_X: f32 = 60.0;
    pub const EMPTY_PADDING_BOTTOM: f32 = 40.0;
    /// How wide the muted line may run before it wraps.
    pub const EMPTY_TEXT_WIDTH: f32 = 420.0;
    /// The Recent row: three cards sharing this width equally.
    pub const RECENT_WIDTH: f32 = 760.0;
    pub const RECENT_CARD_PADDING: f32 = 10.0;
    pub const RECENT_THUMB_HEIGHT: f32 = 96.0;

    // ---- recorder option cards ---------------------------------------------

    /// Each bar control's card, a window of its own floating over the bar.
    pub const RECORDER_CARD_WIDTH: f32 = 320.0;
    /// How far above the bar a card floats.
    pub const RECORDER_CARD_OFFSET: f32 = 14.0;
    /// The rows of a short list — windows, countdown choices, More's items
    /// — sit this close, so the list reads as one object.
    pub const LIST_GAP: f32 = 2.0;
    /// A helper line's leading, relative to its size.
    pub const HELPER_LEADING: f32 = 1.5;
    /// A display tile: its picture, then its name and resolution under it.
    pub const SOURCE_THUMB_HEIGHT: f32 = 66.0;
    pub const SOURCE_TILE_GAP: f32 = 6.0;
    /// A window row's picture.
    pub const WINDOW_THUMB_WIDTH: f32 = 30.0;
    pub const WINDOW_THUMB_HEIGHT: f32 = 20.0;
    pub const WINDOW_THUMB_RADIUS: f32 = 5.0;
    /// The microphone meter: twelve bars of a fixed silhouette, lit from the
    /// left as the level rises.
    pub const METER_HEIGHT: f32 = 26.0;
    pub const METER_BAR_WIDTH: f32 = 3.0;
    pub const METER_BAR_RADIUS: f32 = 2.0;
    pub const METER_GAP: f32 = 3.0;
    pub const METER_BARS: [f32; 12] = [8., 14., 20., 11., 17., 7., 13., 9., 16., 6., 12., 8.];
    /// The quietest level the meter lights a bar for, in dBFS.
    pub const METER_FLOOR_DB: f32 = -60.0;
    /// At or above this the top two bars turn `rec`, and hold for a second.
    pub const METER_CLIP_DB: f32 = -1.0;
    /// The camera card's live picture and the shape swatch set in it.
    pub const CAMERA_PREVIEW_HEIGHT: f32 = 132.0;
    pub const CAMERA_SWATCH: f32 = 44.0;

    /// The strip of tick labels above the lanes. It holds one line of
    /// `FONT_SMALL` and nothing else, so it is that line plus its leading and
    /// not a row's worth of height.
    pub const RULER_HEIGHT: f32 = 16.0;

    /// The depth of the ramp a scroll region fades its clipped edge across,
    /// and the padding that region carries inside itself so the ramp lands on
    /// empty space while nothing is actually clipped. Any box that budgets
    /// height for a scroller has to budget this with it, or the region starts
    /// one band short and fades its own last row for good.
    pub const FADE_BAND: f32 = 16.0;

    /// The lane region's height: the stack the handoff draws — a ruler, the
    /// source lane and five lanes — plus the fade band the scroller rests on.
    /// A project with more lanes than that scrolls inside the region rather
    /// than growing the console into the stage.
    ///
    /// The figure is the region's, not the console's. The console is that
    /// region plus its own padding, its transport row, and whatever else it
    /// is carrying at the time — so a line it only sometimes shows adds to
    /// its height instead of being taken out of the lanes.
    pub const LANE_STACK_HEIGHT: f32 = Self::RULER_HEIGHT
        + Self::LANE_GAP
        + Self::LANE_SOURCE_HEIGHT
        + Self::LANE_GAP
        + 5.0 * Self::LANE_PITCH
        + Self::FADE_BAND;
}

// ---------------------------------------------------------------------------
// typography
// ---------------------------------------------------------------------------

/// The three families, and the one rule that separates them: Geist for the
/// interface, Geist Mono for anything numeric that changes under the user's
/// hand, and Space Grotesk for titles only — never inside a control.
///
/// `FONT_SANS` was `"Helvetica Neue"` until the redesign, so the bundled
/// Geist faces were registered and then never asked for; the app rendered in
/// the system face while claiming otherwise.
pub const FONT_SANS: &str = "Geist";
pub const FONT_MONO: &str = "Geist Mono";
pub const FONT_TITLE: &str = "Space Grotesk";

/// The eight bundled Geist faces, in the order gpui should register them.
pub const GEIST_FACES: [&str; 8] = [
    "Geist.ttf",
    "Geist-Italic.ttf",
    "Geist-Medium.ttf",
    "Geist-MediumItalic.ttf",
    "Geist-SemiBold.ttf",
    "Geist-SemiBoldItalic.ttf",
    "Geist-Bold.ttf",
    "Geist-BoldItalic.ttf",
];

pub const GEIST_MONO_FACES: [&str; 4] = [
    "GeistMono.ttf",
    "GeistMono-Medium.ttf",
    "GeistMono-SemiBold.ttf",
    "GeistMono-Bold.ttf",
];

/// Space Grotesk, weight 500 only: it is a title face, and every title the
/// redesign draws is at 500. Bundling the other weights would ship four
/// faces to serve one.
pub const SPACE_GROTESK_FACES: [&str; 1] = ["SpaceGrotesk-Medium.ttf"];

// ---------------------------------------------------------------------------
// layout metrics
// ---------------------------------------------------------------------------

/// The inspector float.
pub const PANEL_WIDTH: f32 = 340.0;

/// What the stage keeps clear at each side for the floats over it: the float's
/// own inset from the window edge, its width, and the same inset again as air
/// between it and the picture. The preview centres in what is left, so it
/// reads slightly left of the window's true centre — the handoff's choice,
/// over centring in the window and letting the inspector cover the picture's
/// right edge.
pub const STAGE_RESERVE_LEFT: f32 = Theme::INSET * 2.0 + Theme::POD_WIDTH;
pub const STAGE_RESERVE_RIGHT: f32 = Theme::INSET * 2.0 + PANEL_WIDTH;
