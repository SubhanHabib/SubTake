# Native editor visual design

The interface is the "Stage" redesign, delivered as a Claude Design handoff and
extracted into [DESIGN-PRIMITIVES.md](DESIGN-PRIMITIVES.md), which is the
reference any question about a number should be settled against. This document
covers what the redesign means for the app as a whole; the primitive doc covers
what each control measures.

Everything before the GPUI migration — the Slint components under `ui/`, the
September 13 reference screenshots, the 16px control radius, the six-dot
timeline grip — is gone. See [GPUI-MIGRATION.md](GPUI-MIGRATION.md) for that
history.

## What the design is

Four nesting materials over the user's desktop: the window shell (`bg`), a
float that holds controls (`glass`), a float that holds text (`card`), and a
recess inside either (`sunk`). A float's backdrop blur is chosen by what it is
rather than by how big it is — 28 for a pod, 34 for the console and the
inspector, 38 for the recorder bar.

Not drawn by the design: the light `sunk`. The handoff paints it light grey
(`#ececeeb8`). Over the light window material a panel already comes out that
grey, so plates, tracks and lanes disappeared. It is a faint ink instead
(`#14141a12`), which darkens whatever it sits on, as the dark `sunk` does.
`scripts/dev.py --gallery=light` shows it on any `panel-` screen.

**There are no real borders anywhere.** Every edge is a `BoxShadow`: inset for
a hairline, spread for a focus ring. A border adds to what an element measures,
so an edge appearing or changing width would move the element and everything
around it. This is the single rule most likely to be broken by a well-meaning
change.

One blue accent marks exactly four things: the playhead, the selected region or
row, the active tool, and the primary action. A control that is none of those is
grey, which is what lets the four read at all. Selection on a control is an
`accent` inset edge over the fill it already has, never a fill swap.

Red has two jobs and two tokens: `rec` is the only red fill in the app and
belongs to Record; `danger` is red as text, on a destructive label.

Three families: Geist for the interface, Geist Mono for anything numeric that
changes under the user's hand — a timecode, a ruler tick, a slider's value, a
shortcut — and Space Grotesk for titles only, never inside a control.

## Where it lives

`crates/theme` owns every colour, size, radius and gap as a token.
`crates/ui` owns the controls, built only from those tokens; it may not depend
on application state. `src/ui/` composes them into screens. The boundary is
enforced by `python3 scripts/check-ui-primitives.py`, which also rejects a bare
numeric literal wherever a type or icon token belongs.

`crates/ui/src/unused/` holds ten primitives the handoff specifies that the app
has no call site for yet — a field, a stepper, a checkbox, a key cap, a toast, a
dialog, a list row and three menu pieces. They are built to the cards' numbers
and parked rather than left unbuilt.

`SUBTAKE_GALLERY_SCREEN=tour` plays the whole flow on its own in about a minute
and then quits: the recorder cards, the count, recording, pausing and stopping,
then the editor's panels, a selected region, Presets, an export run, and the
inspector folding and sliding at a narrow width. It is there so the flow can be
filmed or watched without anyone clicking.

## The editor

Nothing is docked. That is the change the redesign is named for, and every
other difference follows from it.

A titlebar with no fill of its own — traffic lights, the document on a `sunk`
pill centred on the window, and Record / Presets / Export at the right. Then
one stage filling the rest, with three floats over it: the tool pod at the left
(60 wide, inset 24, centred on the stage's height), the aspect and crop
controls at the stage's top left, and the 340-wide inspector on `card` at the
right.

The pod offers tools — Scene, Cursor, Camera, Captions, Audio — then a hairline
and Settings. Selection has no button: clicking a region opens it in place of
whatever panel was showing, and deselecting goes back to that panel. Not drawn by the
design: the round-2 pod starts with a Sparkle "Zoom" and moves Scene onto the
aspect pod, but neither the Zoom inspector nor that way into Scene is drawn
yet, so the Sparkle still opens Scene; and Help stays under Settings, because
it is the only way to the shortcut reference until the menus are drawn. The stage keeps each float's width clear at its side, so the picture
runs under nothing; the preview centres in what is left and therefore reads
slightly left of the window's true centre. That is the handoff's own answer to
its own open question — the alternative was a larger frame with its right edge
covered.

Under 1280 wide that reserve squeezes the picture, so the inspector folds away
to a 44 round toggle at its top-right corner and the stage takes the width
back — the first of the handoff's two responsive options, which it leaves to
be decided. The toggle, or any tool on the pod, slides the inspector in over
the stage in 240 ms; its close, or the tool already showing, slides it back
out and leaves the panel as it was. Not drawn by the design: the toggle's
icon, which is SlidersHorizontal, and the slide, which the handoff names but
does not time. Scene and Background take a close while folded, since there
closing has somewhere to go. `SUBTAKE_GALLERY_WIDTH=1100` shows the folded
editor in the gallery, and `=inspector-open` the inspector slid in.

A panel with more rows than the float has room for scrolls, its cut edge fading
by as much as is hidden past it. Not drawn by the design: a 4-wide round thumb
in the panel's right padding while it has more than fits, so the fade reads as
"more below" and not as the end (`SCROLL_THUMB_*`). Not wired: dragging it —
it only shows where the view is. `SUBTAKE_GALLERY_SCREEN=export` shows one at
the gallery's size.

Background's wallpaper thumbnails are 48 tall at radius 14, and the one in use
wears the accent ring the handoff gives the picked colour. Not drawn by the
design: each thumbnail's caption, and the faint ring the pointer brings up.
`SUBTAKE_GALLERY_SCREEN=panel-Wallpapers` opens the gallery on them.

Below the stage the console, inset 24 on all three of its edges. Its first row
is the transport: SkipBack, Play on `ink`, SkipForward, the timecode in Geist
Mono, then Suggest zooms / Split / Add as raised buttons, then snap, fit and
the two zoom steps as round icon buttons. The transport belongs here and not on
a pod: the thing that moves the playhead sits on the same surface as the
playhead.

The lane stack is 42 for the source lane and 30 for every other, 4 between
them, with a 78px label gutter 14 from the tracks. A region is its lane's tint
at four strengths: `22` at rest, `2e` under the pointer, `3d` when selected,
`66` for the edge. The playhead is a 2px accent rule the full height of the
stack with a 16px dot and a soft accent ring.

A region's label sits on one line and ends in an ellipsis when the region is
shorter than it. A region under 48 wide shows no label at all, since there is
room only for "…" there. Not drawn by the design: the tooltip that gives every
region its full name. `SUBTAKE_GALLERY_SCREEN=panel-Frame` has both kinds of
region.

The status line has no counterpart in the redesign, which puts progress on the
thing that is progressing. It is carried because export and transcription still
need somewhere to speak, and it lives inside the console, only while it has
something to report. It grows in and folds away over 180ms, keeping its last
words while it folds, so the stage above it eases rather than jumping.
`SUBTAKE_GALLERY_SCREEN=status-cycle` brings it in and out on a timer.

## The empty state

With nothing open the titlebar is empty and there are no floats: no tool pod,
no aspect pod, no inspector. The stage holds one centred column, 26 apart —
"Nothing open yet" in Space Grotesk 40, a muted line, New recording (accent,
hero glow) and Open video (`sunk`) at 52, then a RECENT label and up to three
cards sharing a 760 row. A card is `glass` at radius 22 with a 96-tall
thumbnail at radius 14, a title and a Geist Mono 11 meta line.

In the app the cards are the three newest library entries. The library holds
no thumbnail or running time without opening each file, so a real card says
what kind of file it is and how old it is, and draws a film glyph on `sunk`
where the picture would be. The inspector still opens over the empty state for
the panels that stand on their own — Settings (⌘, or the app menu, since there
is no rail to reach the gear on) and Projects when a recovery is waiting — and
carries a close control there.

## Presets

A centred 470 dialog on `card`, frosted at 38, opened by the titlebar's
Presets button: the title and a close control, the line saying what a preset
does and does not touch, three Appearance rows (a 74×50 preview of the look,
its name, its values in Geist Mono) and two Motion tiles. Picking a row or a
tile only moves the selection; Apply runs the `look-*` and `motion-*`
commands, so browsing never edits the project. The footer is Save current and
Apply at 48.

Not drawn by the design: a Saved section under Motion listing presets saved
to disk, each with a delete control, and Load preset file…. It replaces the
inspector panel Presets used to be.

The dialog's content is on a `layered` layer over its card, and each
selection ring on a layer over its row. Inside a frosted surface every
primitive shares one draw order, and at one order gpui draws all shadows
before all fills, so an inset ring or a glow set on the element it belongs to
goes under that element's fill.

## Export

Export is an inspector panel, not a dialog, so the picture stays in view while
it is set up: a 19 "Export" title and a close control, Video / GIF / Frame,
then OUTPUT (Resolution and Frame rate selects with the value in Geist Mono,
and a Quality slider that reads Low / Medium / High), DESTINATION (the file in
Geist Mono and a raised Change…), the estimated size, and a 52 hero button
named for the format.

The file goes to the Movies folder under the recording's own name — never over
an earlier export there — until Change… picks another. Frame writes a PNG of
the frame under the playhead. The size is a guess from the encoder's settings.

Not drawn by the design: where closing lands (back to Scene, since the
inspector always shows a panel), and the Hardware encoding, Save subtitle
files and Loop GIF switches, kept under the destination for the formats they
change.

Progress is not in the panel. A running export is a 44 pill in the titlebar —
a spinner, "Exporting name.mp4", the percent and time left in Geist Mono, a 3
accent track and Cancel — so the panel can close and editing goes on while the
file is written. When it finishes the pill says "Exported name.mp4" with Reveal
in Finder and a dismiss, and goes by itself after 8 seconds unless the pointer
has been over it. A failure outlines the pill in `danger`, says "Export failed"
with the reason under it, and waits with Try again and dismiss. Cancelling just
removes the pill.

Not drawn by the design: where the pill sits (it takes the document pill's
place while it shows, centred on the window as the document is, growing out
of it over 240 ms and narrowing where it would meet the buttons), and
the failure's headline, which is always "Export failed" because the reason
comes from the encoder as one line and is shown under it rather than split.
Not wired: queueing a second export — Export waits while one runs.

The console's status line no longer speaks for export. Not drawn by the
design: that line itself, kept for transcription and the other background
jobs until toasts are drawn.

`SUBTAKE_GALLERY_SCREEN=export` (or `export-gif`, `export-frame`) opens the
gallery on the panel, where Export runs a fake eight-second export;
`export-progress`, `export-done` and `export-failed` hold the pill in one
state.

## Selection

The region the timeline has selected, named by its kind — a tint swatch the
colour of its lane, "Zoom region" in Space Grotesk 19, and an X that
deselects. TIMING is two timecode fields, Start and End, over the duration in
Geist Mono. A timecode field is typed or scrubbed: click it and the whole value
is selected, so typing replaces it (Return or clicking away keeps it, Esc puts
it back); drag across it and it scrubs 10 ms a pixel, 100 with Shift, the
value going Geist Mono 500 while the hand is on it. A zoom region then has
MOTION — a stepped Zoom level slider through the renderer's six sizes, 125% to
500%, and Follow cursor. A ghost "Delete region" in `danger` with ⌫ ends the
panel. With nothing selected the panel is a plate, "Nothing selected" and
where to click.

Not drawn by the design: every other kind's rows, and a zoom's focus point —
the handoff draws a zoom region only, so the rest keep their rows below the
timing. Not wired: the Ease / Linear / Spring row. A zoom moves on the motion
preset's one curve, so a region has nothing of its own for the three to
choose between.

`SUBTAKE_GALLERY_SCREEN=selection` opens the gallery on a zoom region's
panel, `selection-empty` on the empty state; clicking any region opens it.

## Cursor

A 19 "Cursor" title and a close control, then Show cursor, the STYLE tiles,
Size, CLICK EFFECT as one segmented control, and MOVEMENT — Smooth movement
and Smoothness. With Show cursor off every row under it dims to 0.4 and takes
no clicks; Smoothness dims the same way while Smooth movement is off. Smooth
movement is not a setting of its own: off sets the smoothing to 0, on puts
the default 0.67 back, and Smoothness is that same number.

Not drawn by the design: the style picker, whose five tiles are the
renderer's styles (the handoff's Arrow / Hand / Dot are not), where closing
lands (Scene), and a More section with the rest of the cursor settings —
sway, motion blur, the click effect's size, opacity, length and colour,
bounce, looping and the camera's motion blur. The motion presets moved out:
the Presets dialog owns them. Palette churn: Click effect is None / Ripple /
Spotlight / Echo, because the renderer has no Pulse. Not wired: Tab still
reaches the dimmed rows.

`SUBTAKE_GALLERY_SCREEN=cursor` (or `cursor-hidden`, or `cursor-unknown` for a
style with no tile) opens the gallery on it.

## Camera

The webcam overlay, called the camera in the interface. A 19 "Camera" title
and a close control; POSITION as four 60 `sunk` tiles two across — bottom
right, bottom left and top right, each with a 16 dot in its corner (`accent`
when chosen), then Custom — the chosen one ringed by a 1.5 accent edge; SHAPE
as Circle / Rounded / Square; Width; and Mirror. Custom starts where the
overlay already is, so choosing it moves nothing. The shapes are the
overlay's roundness at 100, 25 and 0.

Not drawn by the design: the footage row and Show webcam, which come first
and dim the rest while the overlay is off; the other six position presets a
project can hold (they still place it, and no tile is ringed); where closing
lands (Scene); and a More section with height, the custom position, margin,
shadow, reacting to zoom, the crop and the time offset. Palette churn: Width
reads as a percentage of the frame's shorter side, which is what the model
stores, not in points; and a pressed tile darkens rather than scaling to
0.98, since gpui has no element transform. Not wired: dragging the overlay
on the stage for Custom (the position rows under More set it), and the
helper line about the camera being its own track — it is not a track yet.

`SUBTAKE_GALLERY_SCREEN=camera` (or `camera-off`) opens the gallery on it.

## The recorder

A compact rounded floating bar with a source control, audio and webcam options,
a countdown, a red Record control, More and Hide. Option panels expand above it.

The bar keeps its size and shape in every state; only its controls change.

- **Counting down** — the count in a 60 round plate, "Recording starts in 3…"
  over the source and whether the mic is on, and Cancel (`esc`).
- **Recording** — the grip, a red pill with a pulsing white dot and the elapsed
  time in Geist Mono, Pause and Stop, then whether the mic and camera are in
  the capture (the glyph says it, not a colour) and a red X that discards it.
- **Paused** — the pill turns `sunk` with a dimmed dot and PAUSED; Pause
  becomes a red Resume. Nothing else moves.
- **Stopping** — a spinner, "Finishing your recording" over how much was
  captured, and a plate (not a button) saying it opens in the editor.

While the count runs it is also drawn on the display being recorded (a window
source counts on the display its centre is on): the screen dims, the number
sits in the middle in 96 white Geist Mono, shrinking to 0.86 and fading over
each second, with a "Press esc to cancel" chip under it and a white frame 8 in
from the edges. That window takes no clicks and sits just under the bar. Esc
cancels from anywhere: SubTake holds it as a global shortcut only while the
count runs, and gives it back when it ends. Palette churn: the numeral has no
shadow and the chip no blur — gpui has no text shadow, and a click-through
sheet has no material to blur.

Not drawn by the design: the other waits — finding displays, starting
capture, holding or resuming it — take the Stopping layout with the status
line, and Cancel when there is something to cancel. Palette churn: PAUSED is
not tracked out, because gpui sets no letter spacing.

`SUBTAKE_GALLERY_SCREEN=rec-counting` (or `rec-recording`, `rec-paused`,
`rec-stopping`) opens the gallery with the bar in that state — `rec-counting`
with the on-screen count too; Record in the gallery runs the whole sequence.

The recorder's windows are borderless, and macOS gives a borderless window no
corner mask, so the editor's window material would fill the frame square behind
a rounded plate. Instead each recorder window gets a native material masked to
exactly its plate — the whole window at `RADIUS_BAR` — and the plate paints the
handoff's `glass` tint over it, so glass and tint are one surface. The window
server's shadow is off: it rings a transparent window with a dark rim, which on
a plate that fills its window reads as an outline.

### Option cards

Each bar control opens a 320 card 14 above the bar, centred over the control
that opened it and kept inside the bar and the screen: a `Recorder / name`
chip and a close control, then the card's rows, then a muted helper line.

- **Capture source** — Displays as two picture tiles (the chosen one ringed in
  accent inside and out), Windows as 34 rows with a 30×20 picture, and Refresh.
- **Audio** — the microphone select, Record microphone, a 12-bar level meter
  with its peak in dB (the top two bars turn red for a second on a clip), and
  Record system audio.
- **Camera** — Webcam overlay, the camera select, and a 132 preview with the
  overlay's shape in its corner.
- **Countdown delay** — four 44 rows; the chosen one is sunk, ringed, checked.
- **More** — Open, Projects, Back to editor, then the recordings folder.

The card measures its own height and the window follows, so a card never
clips and never leaves empty glass. Its window glass is masked at
`RADIUS_PANEL`.

Not drawn by the design: the "Create video · spike" row in More, the empty
Sources state, the camera card with no camera, and the Refreshing… pill.
Not wired yet: source pictures, the microphone level and the camera preview —
outside the gallery they show a glyph, "— dB" and the camera glyph.

Palette churn: the Preview chip on the camera picture keeps its tint but not
the handoff's 18 backdrop blur.

`SUBTAKE_GALLERY_SCREEN=card-sources` (or `card-audio`, `card-camera`,
`card-countdown`, `card-more`) opens the gallery with that card up.

## Gestures

Pinch over the timeline zooms between 1x and 100x, anchoring the time under the
pointer. Pinch over the stage magnifies from Fit to 8x, anchoring the image
point under the gesture; two-finger scroll pans, and Fit resets both. All of it
is viewport state — none of it touches project content, the playhead, or the
exported framing. The Fit pill's percentage is set in tabular figures so the
pill holds its width while a pinch runs. `SUBTAKE_GALLERY_SCREEN=zoom-111` (any
percentage) opens the gallery magnified.

## Motion

Not drawn by the design: every transition below. The handoff draws still
screens; these were chosen on a motion board and live as tokens in
`crates/theme/src/metrics.rs`. All of them honour the Mac's Reduce motion
setting and land in place when it is on.

- A picked inspector panel fades in and rises 6 (`PANEL_ENTER_*`, 180 ms).
  Into a sub-panel (Crop, Background, Shortcuts) it slides in from 24 to the
  right instead, and back out to its parent from the left (`PANEL_DRILL_*`).
- A recorder card grows up out of the bar as it opens (`CARD_OPEN_MS`) and
  folds back as it closes (`CARD_CLOSE_MS`); a swap to a taller or shorter
  card eases between the heights (`CARD_RESIZE_MS`). The options window
  holds the taller height while it eases, and its frosted material follows
  the card rather than the window. The frost never shows past the card's
  edge: it takes the card's lowest height over `CARD_GLASS_SKEW_MS` either
  side of now, since the two reach the screen by different routes, and a
  closing card drops it at once. The open card holds keyboard focus so it
  eases at the display's rate, and hands it back to the bar as it hides.
  The frost takes the app's light or dark theme, not the Mac's.
  The card's window opens hidden and shows once it sits above the bar, and
  each resize puts it back above the bar in the same move. Under Reduce
  motion a just-opened card stays undrawn until its window has grown to
  fit, so it appears whole. A swap under Reduce motion holds the old
  card's plate empty until the new card's rows are measured and the window
  fits them, rather than drawing the new rows at the old height.
- The recorder bar keeps its size and place; when it turns from ready to
  counting to recording to writing, its controls fade in (`BAR_SWAP_MS`).
- Pausing eases the clock from red to `sunk` and dims its count to 60%
  (`PAUSED_CLOCK_OPACITY`); Pause and Resume fade in as they swap.
- The Presets dialog fades in rising 12 and fades back out
  (`DIALOG_*`); its frost eases with it, since opacity does not reach a
  backdrop blur.
- Dropdowns and the command menu drop 4 as they open and fade out over
  `MENU_OUT_MS` when dismissed. Their entrance settles on a quint, not the
  ease-out every other move uses, so a menu is there the moment it is asked
  for.
- A finished export's tick draws itself on, its label fades in, and the pill
  gives one soft accent pulse (`EXPORT_DONE_*`).

Not wired: the card's sideways move when it swaps to a control further
along the bar — the window is placed natively and jumps.

## Control states

Every control that does something has four states, the ones a button has.
Under the pointer it takes a hover wash that fades in and out
(`motion::hover_blend`). While held it dims to `PRESSED_OPACITY`, on the
`press` fill unless its own fill already says it is picked. When the keyboard
brings focus to it, the `accent_soft` ring appears (`focus_visible`), with no
ring after a click. Disabled, it dims to `DISABLED_OPACITY` and Tab passes it
by. Hand-built rows, tiles and pills get all four from `subtake_ui::pressable`.
`SUBTAKE_HOVER_PIN=<words>` holds a hover on in the gallery.

Not drawn by the design:

- the hover washes on the switch, text fields, colour swatches, the
  titlebar's document pill, the Recent cards and Resume (a white lift over
  `rec`);
- a selected timeline region's hover (`REGION_FILL_SELECTED_HOVER`) and a
  held trim handle's full-tint mark;
- every pressed and focused look.

The focus ring snaps on everywhere, the buttons included; only hover fades.
The document pill takes a hover and nothing else, since it only moves the
window. The microphone and camera marks on the recording bar are
indicators, not controls, so they take no state at all.

Text fields keep the Mac's editing keys. ⌘A selects all. ⌃A and ⌃E go to the
line's ends, as in every Cocoa field. ⌥ moves and deletes by word, ⌘ by line,
⌘Z and ⇧⌘Z undo and redo, and a double or triple click selects a word or
everything. Other platforms get the Ctrl equivalents. Up and down walk the
command palette's highlight; Enter runs it.
