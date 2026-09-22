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

## The editor

Nothing is docked. That is the change the redesign is named for, and every
other difference follows from it.

A titlebar with no fill of its own — traffic lights, the document on a `sunk`
pill centred on the window, and Record / Presets / Export at the right. Then
one stage filling the rest, with three floats over it: the tool pod at the left
(60 wide, inset 24, centred on the stage's height), the aspect and crop
controls at the stage's top left, and the 340-wide inspector on `card` at the
right. The stage keeps each float's width clear at its side, so the picture
runs under nothing; the preview centres in what is left and therefore reads
slightly left of the window's true centre. That is the handoff's own answer to
its own open question — the alternative was a larger frame with its right edge
covered.

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

The status line has no counterpart in the redesign, which puts progress on the
thing that is progressing. It is carried because export and transcription still
need somewhere to speak, and it lives inside the console, only while it has
something to report.

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
place while it shows, centred between the traffic lights and the buttons), and
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
exported framing.
