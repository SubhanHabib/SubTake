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

Not drawn by the design: the shell is 40% (`#fafaf966`, `#0d0d1066`) and
`glass` and `card` are at about half the handoff's alpha (dark `glass` a
little more, at 41%), so the desktop shows through every surface. `scripts/dev.py --gallery=light` shows them on any
`panel-` screen.

Also not drawn by the design: `ground` (`#e6e6e9`, `#1a1a1f`) is the opaque
solid under the window's material. macOS drops the live blur in Mission
Control's thumbnails and for a frame or two while it rebuilds the blur going
into and out of them; the window shows `ground` under `bg` there, where it
used to flash black.

**There are no real borders anywhere.** Every edge is a `BoxShadow`: inset for
a hairline, spread for a focus ring. A border adds to what an element measures,
so an edge appearing or changing width would move the element and everything
around it. This is the single rule most likely to be broken by a well-meaning
change.

Two drop shadows lift a float, each a wide layer and a tight one, and both go
deeper on dark. The window's own planes — the console, the inspector, the tool
pod, the aspect pod and the titlebar's pills — take `plane_shadow()`
(`shadow_plane_far` / `shadow_plane_near`, `PLANE_SHADOW_*`); a dialog, a menu
or a dropdown takes `panel_shadow()` (`shadow_far` / `shadow_near`,
`PANEL_SHADOW_*`). `Surface::Panel` and `Surface::Pod` pick the first,
`Content` and `Popup` the second. Not drawn by the design: the split is so the
planes tune as one without moving a menu's shadow; the two start equal.

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

A titlebar with no fill of its own — traffic lights, the document on a pill
centred on the window, and Record / Presets / Export at the right as 44
round icon-only controls, each named by its tooltip: Record and Presets on
`sunk` plates, Export on the accent. Not drawn by the design: they sit 10 in
from the window's right, the same as their 10 from its top
(`TITLEBAR_PADDING` is derived from the titlebar and control heights), so they
sit square in the corner. Not drawn by the design: the handoff
gives Record and Export their captions. While the document has a status — a
running job, an error, "Gallery mode" in the gallery — it is a chip at the end
of the title pill: 26 tall, `sunk2`, the small size in `muted`, 12 after the
title to match the pill's left padding, with the pill's right padding closing
to 6 around it. Not drawn by the design: a running job's
small X, which cancels it, and the chip's tooltip with the whole status. Also
not drawn by the design, whose pill is `sunk` on the window's own ground: the
pill, and the export pill in its place, is a pod's frosted `glass` with its
hairline and shadow, hovering to `card`, since the stage and the desktop run
under the titlebar and a `sunk` tint over them left the title unreadable. Then
one stage filling the rest, with three floats over it: the tool pod at the left
(60 wide, inset 24, centred on the stage's height), the aspect and crop
controls centred along the stage's bottom, and the 340-wide inspector at the right on
`glass`, as the timeline console is. The handoff puts the inspector on `card`; `glass`
is a departure from it, so the two floats read as a pair.

The pod offers tools — Scene, Cursor, Camera, Captions, Audio — then a hairline
and Settings. Selection has no button: clicking a region opens it in place of
whatever panel was showing, and deselecting goes back to that panel. Not drawn by the
design: the round-2 pod starts with a Sparkle "Zoom" and moves Scene onto the
aspect pod, but neither the Zoom inspector nor that way into Scene is drawn
yet, so the Sparkle still opens Scene; and Help stays under Settings, because
it is the only way to the shortcut reference until the menus are drawn. In a
window too short for it, the whole pod scrolls as one, Settings and Help with
the tools, cut hard at the glass's edge (`SUBTAKE_GALLERY_HEIGHT=640`). The stage keeps each float's width clear at its side, so the picture
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

The inspector's left edge and the console's top edge each take a drag: the
inspector from 300 to 520 wide, and the lane region from the ruler and two
lanes up to half the window, whatever lanes the project has. The stage
gives or takes the difference, and a double click puts either back at rest
(340, and the clip lane, one lane and half of a third, the half telling there is more below). A 36×4 grip fades in on the edge under the pointer and
stays while it is dragged. The drag follows the pointer over the floats as
well as the stage, so it runs both ways. Not drawn by the design: the handoff's floats
are fixed. The sizes are remembered between launches, and Settings →
General → Editor layout puts both back at rest at once (its Reset is off
while they already are).
`SUBTAKE_GALLERY_LANES=100 SUBTAKE_GALLERY_INSPECTOR=460` starts the gallery
resized, and `SUBTAKE_HOVER_PIN=resize` shows the grips.

A panel with more rows than the float has room for scrolls. Its top fades by as
much as is hidden past it; its bottom runs to the glass's edge and cuts there,
unfaded, unless a footer closes the panel, when it fades into the footer.
Not wired: cutting on the glass's own curve. gpui clips only to rectangles,
so a row scrolled into a rounded corner, or a tool-pod button into the pod's
round end, is cut flat at the edge line; clipping to the radius needs a
rounded clip in the renderer's shaders. Not drawn by the design: a 4-wide round thumb
in the panel's right padding while it has more than fits, so the fade reads as
"more below" and not as the end (`SCROLL_THUMB_*`). Not wired: dragging it —
it only shows where the view is. `SUBTAKE_GALLERY_SCREEN=export` shows one at
the gallery's size.

Each edge's fade can be turned off (`fade_edges(..).top(false)` /
`.bottom(false)`), cutting hard at the edge instead. The console's lanes fade
at the top and cut hard at the bottom, where the console's edge already closes
them. `SUBTAKE_GALLERY_LANES=130` shows the cut,
and `SUBTAKE_GALLERY_HEIGHT=850` with `=panel-Frame` shows a panel's.

A new recording opens on Apricot: a 135° peach-to-rose gradient
(`#f7dcc2`, `#eda88f`, `#d27b86`), padding 56, radius 4 and shadow 50%. The
shadow is three layers — a contact shadow at the edge, a key shadow under it
and a wide ambient one — tinted `#5c2224` from the wallpaper rather than
black, with a 10% hairline just inside the frame's edge. Not drawn by the
design: the handoff gives no default scene. Not wired: `shadowColor` and
`frameEdgeColor` have no control in Scene; a new project and the built-in
looks set them (the looks back to black with no hairline).

Background's wallpaper thumbnails are 48 tall at radius 14, and the one in use
wears the accent ring the handoff gives the picked colour. Not drawn by the
design: each thumbnail's caption, and the faint ring the pointer brings up.
`SUBTAKE_GALLERY_SCREEN=panel-Wallpapers` opens the gallery on them.

Below the stage the console, inset 24 on all three of its edges. Its first row
is the transport: SkipBack, Play on `ink`, SkipForward, the timecode in Geist
Mono, then Suggest zooms / Split / Add as raised buttons, then snap as a round 40 icon
button — a `sunk` plate and a `text` glyph while on, no plate and a `muted`
glyph while off, never the accent — and the zoom control — the aspect pod's own (`zoom_control`), reading
the timeline's zoom as a percentage of the whole take, stepping by half again
up to 100x. Not drawn by the design: the handoff draws fit and the two steps
as three round icons. The transport belongs here and not on
a pod: the thing that moves the playhead sits on the same surface as the
playhead.

The timeline is option 3a of the seek bar handoff (`Timeline 3a.md`). It is
two columns 10 apart: a 44-wide column of lane headers and the track column.
The track column starts with a 34 band for the playhead's bubble, then the
ruler, 10 of air and the lanes, 8 apart. Lanes are 44 and the clip lane 52,
top to bottom Zoom, Clip, Annotation, Caption, Audio; speed and trim regions
sit in the clip lane. There is no track under a lane, no text gutter and no
source lane: regions float on the console's glass, the headers name the lanes,
and the recording's frames are drawn inside its clips. A header is a 44 circle
on `sunk` with a hairline and the lane's glyph at 17 in `text`
(`MagnifyingGlassPlus`, `FilmStrip`, `TextT`, `ClosedCaptioning`,
`MusicNotes`), centred on its lane. A press on a header turns its lane off, or
on again: the audio lane is muted and every other lane hidden. Off, the header
loses its plate and its glyph goes `muted` with a 1.5 slash 22 long at −45°
through it, and the lane's regions show at 40%, still selectable and editable.
Under the pointer a header goes to `sunk2`, off or on; pressed, to
`press`; focused from the keyboard, it takes the 3 `accent_soft` ring. Its
tooltip names the lane and its state, `Captions · hidden`.
`SUBTAKE_GALLERY_SCREEN=lanes-off` has the captions hidden and the sound
muted. Not drawn by the design: the press's 0.94 scale; and an off header does
not blur a lane running under it. Off is an edit, undone like any other, and
the project saves it in `lanesOff`. The preview and the export leave a hidden
lane out: no zooms, annotations or captions, and a hidden clip lane takes the
recording, its shadow and the cursor off the wallpaper. A muted audio lane
plays and exports silence. Not wired: the right-click menu (Lock, Solo, Remove
lane).

A lane with nothing on it is not drawn, header included; adding a region of
its kind brings it back. `SUBTAKE_GALLERY_SCREEN=lanes-sparse` has only zooms
and the recording's sound, with an imported voice-over on a second audio lane.
Not drawn by the design: a lane that overflows onto a second row shares the
first row's header; and the take itself — its kept spans in the clip lane
while the project has no clips of its own, and its sound first in the audio
lane (`Region::TAKE_CLIP`, `TAKE_AUDIO`). Neither is a project region, so a
press on one seeks rather than selecting it. Not wired: dragging a new region
from Add over the timeline to show its lane. Zoomed in, the lanes run on past
the track column, out to the console's own edge on the right and under the
headers on the left, as a region's pill runs under its plate; the ruler keeps
to the track column. The headers blur what runs under them by 16. A lane is
cut half its height before its header's centre, so what runs past it ends in
a round end on the header's circle, a clip lane's 4 past it all round for its
extra height. Its label and frames stay put rather than sliding along, a frame
cropped at the cut, and its start has no trim handle while it is under the
header. Not drawn by the design: gpui clips only to a rectangle, so what sits
over a lane's fill, its label, plate, sound and chip, is cut straight at the
header's centre, where the circle covers it; a clip's frames are rounded or
shortened to stay inside the round end, and a cut frame narrower than the
lane is tall is drawn that wide under the next, its picture slid along.
What is left of a lane narrower than it is tall is a circle that wide,
centred, one frame for a clip, rather than a sliver the lane's height, which
gpui would round no more than half its width.
`SUBTAKE_GALLERY_SCREEN=timeline-zoomed` shows the timeline at 300%, 30
seconds in. Not drawn by the design, whose lanes stop at the track column.

A region is a pill the lane's height, a solid fill with an ink, both from its
lane's hue: in light the fill at 75% saturation and 82% lightness under an ink
at 65% and 25%, in dark the fill at 35% and 34% under an ink at 60% and 88%.
It floats on the console's glass with no edge at rest. 4 in from its start
sits a `plate` circle 4 short of the lane at each end (36, or 44 in the clip
lane) with the kind's icon at 16 in the ink — `MagnifyingGlassPlus`, `Timer`,
`Scissors`, `TextT` (`ArrowUpRight` for an arrow), `ClosedCaptioning`,
`MusicNotes`, `FilmStrip` — then, 10 on, the label in Geist 500 at 13 in the
ink, 14 short of the end. Selected, it takes a 2px accent ring and a trim
handle at each end: a 5 by 18 accent bar, radius 3, with a 1.5 white ring,
standing 3 out past the end. Under the pointer the handles show at half
strength; the one being dragged grows to 6 by 22 at full. A region being moved
lifts on the panel shadow over a 1px dashed `line` outline where it started.
The recording's sound fills the rest of its region as bars, 10 after its label
in a 26 row: 2.5 wide at radius 2 on a 4.5 pitch, each 22% to 94% of the row
by the loudest peak in its span, in the ink, and at 35% once past the
playhead. `SUBTAKE_GALLERY_SCREEN=selection` has a selected zoom; add
`SUBTAKE_GALLERY_GESTURE=move` or `trim` to see it moved or its end handle
held, or `scrub` for the playhead held. Not drawn by the design: a native
marker, which has no icon and so no plate. Not wired: an imported sound's
waveform, which is never read; and the peaks, which are read back from
ffmpeg's waveform picture of the take, one per column of its 1280, rather than
decoded.

With the magnet on, a region snaps as it is dragged, so it shows where it
will land: whichever edge being dragged comes within 6 of another region's
edge, the playhead or either end of the take lands on it. A move drags both
edges and the nearer catches; a trim drags one. Regions moving with it, the
rest of the selection, are not caught. With none of those in reach, the edge
catches the ruler's marks, labels and dots alike, so a drag lands on whole
seconds zoomed out and on finer steps zoomed in; an edge in reach always wins
over a mark. Where an edge has caught, a 1 guide in `text` at 50% runs down the
ruler and the lanes, under the playhead; a mark caught shows none, since the
dots already mark it. `SUBTAKE_GALLERY_SCREEN=selection
SUBTAKE_GALLERY_GESTURE=snap` holds the selected zoom moved so its end has
caught the next edge after it. Not drawn by the design, which has
the magnet and no behaviour for it. ⌘ held drags freely, magnet or not.

A clip shows the recording's frames rather than its tint: tiles 88 wide and
the lane's height, each followed by a 2 divider in black at 35%, the first and
last rounding the clip's ends. A last tile too short to take the curve is
folded into the one before it; the tint shows while the frames load. Its name
sits on a chip 4 in from the clip's start, top and bottom: a `card` pill with
a hairline over the frames blurred by 16, a 36 `plate` with `FilmStrip` at 15
in `text`, then, 8 on, the name in Geist 500 at 13 in `text`, 14 short of the
chip's end. A clip too short to leave the name four letters shows the chip's
plate alone. A speed region is labelled by its speed, `2×`. Not drawn by the
design: a tile's frame is the recording's at the tile's place on the timeline,
not at its place in the clip's source; the chip's `saturate()`, which gpui has
no filter for, and its blur where the window has no glass; and a clip shorter
than the chip, which shows frames alone. Not wired: the frames are ten
thumbnails across the take, stretched to the tiles, rather than a cache
decoded at 88 by 52 per tile and kept across zoom levels.

The ruler is a 32-tall `sunk` band, fully round, with a hairline. It counts in
`m:ss`, Geist Mono at 11, at the smallest of 1, 2, 5, 10, 15, 30, 60, 120 or
300 seconds that keeps its labels 80 apart, recomputed as the timeline zooms;
between them a 3 dot marks every fifth of that step. Labels behind the
playhead are `text` and dots `text` at 70%; ahead of it, `muted` and `muted`
at 40%. A press anywhere on the band jumps the playhead there. Not drawn by
the design: a dot that would touch a label is left out, which at the closest
spacing leaves two dots between labels rather than four.

The playhead is four parts on one x. Its time, `m:ss.cc` in Geist Mono 500 at
12 in `on_accent`, sits on a 26-tall accent bubble at the top of the track
column, with a 10 by 6 tail pointing down at the ruler; near either end the
bubble slides to stay inside the column while the tail stays on the time. A
9 accent dot with a 3 `accent_soft` ring sits on the ruler's top edge, and a
1.5 accent line runs from there to the bottom of the stack over every
region. A 12 by 40 accent handle with three `on_accent` grip dots sits
centred on the clip lane. The bubble, line and handle glow in `accent_glow`
(18, 10 and 12). Dragging the bubble, dot, handle or line (6 either side,
with an `ew-resize` cursor) scrubs, and the playhead keeps its offset from
the pointer rather than jumping to it; while scrubbing the handle widens to
14, its glow to 16 and the bubble's to 24. Not drawn by the design: with no
clip lane the handle sits on the first lane. Not wired: the app's transport
still reads milliseconds (`00:37.150`); the gallery's reads hundredths, as
the bubble does.

A region's label sits on one line and ends in an ellipsis when the region is
shorter than it. A region that would leave its label fewer than four letters
shows only its plate, centred, 4 in at both ends. The tooltip names the
region and gives its range in Geist Mono at 60%, `2×  0:33–0:41`. Not
drawn by the design: a region too short for its plate shrinks the plate to
fit, and drops the icon once the plate is smaller than it; the tooltip shows
on every region, not only a cut or plate-only one, since whether a label is
cut is only known after layout; and it sits by the pointer rather than 8
above the region, centred, as gpui places tooltips.
`SUBTAKE_GALLERY_SCREEN=panel-Frame` has both kinds of region.

The console has no status row: the document's status is the title pill's
chip. `SUBTAKE_GALLERY_SCREEN=status-cycle` swaps the chip between a running
transcription and "Gallery mode" on a timer. With nothing open there is no
title pill, so the empty state keeps a status line under the stage, growing in
and folding away over 180ms. Not drawn by the design: that empty-state line.

## The empty state

With nothing open the titlebar is empty and there are no floats: no tool pod,
no aspect pod, no inspector. The stage holds one centred column, 26 apart —
"Nothing open yet" in Space Grotesk 40, a muted line, New recording (accent,
hero glow) and Open video (`sunk`) at 52, then a RECENT label and up to three
cards sharing a 760 row. A card is `glass` at radius 22 with a 96-tall
thumbnail at radius 14, a title and a Geist Mono 11 meta line.

In the app the cards are the three newest library entries. A card's picture
is one frame a quarter of the way into the take (a project's source video),
made in the background and cached beside the timeline artwork; until it
arrives, or when it cannot be made, the card draws a film glyph on `sunk`. The
library holds no running time without opening each file, so the meta line
says what kind of file it is and how old it is. The inspector still opens over the empty state for
the panels that stand on their own — Settings (⌘, or the app menu, since there
is no rail to reach the gear on) — and carries a close control there.

## Projects

Not drawn by the design, which stops at the three Recent cards. All projects,
a ghost button at the end of the RECENT label's row, opens the Projects view:
the whole library on the stage, not in the inspector. It takes the stage over
the empty state or an open take alike (the recorder's More → Projects opens it
too), with no pods, inspector or timeline over it, and opening a card brings
that take up in the editor.

A heading row, 1000 wide like the grid under it: a back caret, "Projects" in
Space Grotesk 19, then a 240 search field, the project folder (its name, with
the folder glyph; a press chooses another), Refresh and Open…. Under it, an
UNSAVED RECOVERY label and one small button per recovery when any are waiting
(the reason the view opens at launch after a crash), then an "N IN THE
LIBRARY" label and the library as Recent cards, four to a row, scrolling with
faded edges. The scroll region spans the stage so its clip never cuts the
cards' shadows. With nothing to show it says so in the empty state's voice.

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

Each selection ring is a `frost::edge` over its row or tile and the preview
frame is `layered` over the preview, so a look's shadow shows. Inside a
frosted surface every primitive shares one draw order, and at one order gpui
draws all shadows before all fills, so an inset ring or a glow set on the
element it belongs to goes under that element's fill (see Frosted glass in
UI-COMPONENTS.md).

## Export

Export is an inspector panel, not a dialog, so the picture stays in view while
it is set up: a 17 "Export" title and a close control, Video / GIF / Frame,
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

The title pill's status chip does not speak for export; the export pill
does.

`SUBTAKE_GALLERY_SCREEN=export` (or `export-gif`, `export-frame`) opens the
gallery on the panel, where Export runs a fake eight-second export;
`export-progress`, `export-done` and `export-failed` hold the pill in one
state.

## The Add popup

Kept beside the Add panel for now, so the two can be compared; the Add
button opens the popup. A 560-wide frosted card over the button (above it
when there is no room below, its body shortening from 360 to 316 rather than
covering the button): the palette's field, led by + and ended by an esc cap,
"Add at" the playhead's time, that filters as it is typed, its match in the
accent and semibold; the kinds as glyph-led command rows in three menu
sections, Timeline, Picture and Sound and words, each row's shortcut key cap
at its end while nothing is typed, the list scrolling down to the footer's
rule and cut hard there, not faded; and beside them a card for the highlighted
kind — the picture with it on, the lane it lands on with the time in Geist
Mono and a strip of the regions already there ±8 s with the new one at the
playhead, what it is for, and "Add zoom ↵" at its foot, level with the
list's. ↑↓ move, Return adds, Esc
closes; the footer says so beside the section switcher. With no video the
list is dimmed and the card offers Open….

Not drawn by the design: the picture is the take's frame at the playhead, not
a stand-in; a trim is drawn as the accent at half strength rather than
hatched; adding closes the popup, so the footer never says what was added.

`SUBTAKE_GALLERY_OPEN=menu-Add` opens the gallery on it, `menu-Add:sp` with
"sp" typed; `SUBTAKE_GALLERY_SCREEN=empty` with it shows the no-video state.

## Selection

The region the timeline has selected, named by its kind — a tint swatch the
colour of its lane, "Zoom region" in Space Grotesk 17, and an X that
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

A 17 "Cursor" title and a close control, then Show cursor, the STYLE tiles,
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

The webcam overlay, called the camera in the interface. A 17 "Camera" title
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

The microphone and the camera say whether they are live by the same rule on
every bar, idle, recording and paused: on is a 60 round `sunk` plate under a
`text` glyph, off no plate under the struck-through glyph
(`MicrophoneSlash`, `VideoCameraSlash`) in `muted`. The grip, More and Close
are `text` at full strength. `SUBTAKE_GALLERY_INPUTS=off` starts the gallery
with both off, on any of those screens.

The recorder's windows are borderless, and macOS gives a borderless window no
corner mask, so the editor's window material would fill the frame square behind
a rounded plate. Instead each recorder window gets a native material masked to
exactly its plate — the whole window at `RADIUS_BAR` — and the plate paints the
handoff's `glass` tint over it, so glass and tint are one surface. The window
server's shadow is off: it rings a transparent window with a dark rim, which on
a plate that fills its window reads as an outline.

### Option cards

Each bar control opens its card 14 above the bar, centred over the control
that opened it and kept inside the screen: a 40 header with the card's name
and what it is set to (the summary), then the card's rows, then a muted
helper line. Source is 380 wide, Microphone and Camera 340, Countdown 320
and More 360.

- **Source** — three tabs, opening on the chosen source's. Display: the
  displays as 16:10 pictures, the chosen one ringed and checked, then Hide
  desktop icons and Show recorder in capture. Window: a search, then the
  windows two across. Area: the area drawn, over its display's picture
  dimmed around it with its outline, corner dots and size; the Aspect lock;
  and Draw area on screen, which opens the area overlay (below).
- **Microphone** — Record microphone, the device, a level meter, the Input
  level (a gain at capture, not the device's own volume), Test, and Record
  system audio.
- **Camera** — the live picture, mirrored; the camera; its corner, shape and
  size in the recording.
- **Countdown** — four tiles for the delay, and the tick.
- **More** — the recent projects, Open and All projects, then the
  recording's resolution, frame rate and folder, Hide bar while recording
  and the record shortcut.

A permission the system refuses turns the card into its notice with Open
Settings; no device found, or the chosen one unplugged, reads in the
summary and the card.

The card measures its own height and the window follows, so a card never
clips and never leaves empty glass. Its window glass is masked at
`RADIUS_PANEL`.

Palette churn: the chips over the camera picture and the area's size chip
keep the `frost` tint without the handoff's 16 backdrop blur.

#### Area overlay

Draw area on screen hides the card and the bar and dims every display under
`rgba(10,12,20,.48)`. Dragging draws an area; once drawn it is moved by
dragging it and resized by its 10 corner and 8 × 8 side handles, ⇧ keeping
the ratio (a square when Aspect is Free) and the arrows nudging it (⇧ by
10). The size sits in a frosted chip 10 under it, and Cancel and Use area
under that; Return or a double-click uses it, Esc cancels. Either way the
bar and the Source card come back, the area chosen if one was used.

Not drawn by the design: pressing without dragging takes what the pointer
is over. As it moves the overlay offers the element under it — the page in
a browser without its toolbar, a pane, the window, the display — ↑ and ↓
or a scroll widening and narrowing the offer, its name beside the size in
the chip, and a hint along the top says so. The offers come from the
window list, Accessibility when SubTake is already trusted (it never asks),
and the long edges in a still of each display. While drawing, moving or
resizing, a side near one of those edges is drawn onto it and the edge is
shown; ⌘ draws freely.

The area is a source of its own, listed after the displays and windows and
kept across launches: its display's stream, cropped to it, fitting the
area rather than the display under the chosen resolution. The handoff's
"Area · not set" summary and disabled Record are never reached, since the
area only becomes the source once one is drawn.

`SUBTAKE_GALLERY_SCREEN=card-sources` (or `card-audio`, `card-camera`,
`card-countdown`, `card-more`) opens the gallery with that card up;
`card-sources-window` and `card-sources-area` on those tabs,
`card-sources-area-set` with an area drawn and chosen, and
`card-more` with `SUBTAKE_GALLERY_OPEN=resolution` with the resolution menu
down. `-denied` (`card-audio-denied`), `-none`, `-unplugged`,
`card-audio-listening`, `card-audio-playing` and `card-camera-starting`
hold those states. `area-overlay` opens the overlay with an area drawn,
`area-overlay-picking` with none and the pointer's offer, and
`area-overlay-locked` held to 16:9. `rec-working`, `rec-hidden` and
`rec-hidden-cycle` show the bar waiting on something it can cancel, and
hidden to its clock while recording, still and with the pointer coming and
going.

## Gestures

Pinch over the timeline zooms between 1x and 100x, anchoring the time under the
pointer. Pinch over the stage magnifies from Fit to 8x, anchoring the image
point under the gesture; two-finger scroll pans, and Fit resets both. A
magnified picture is not cut off at the stage: it runs on under the titlebar,
the pods and the console to the window's edge, and shows through their frost,
while the pan still stops where the picture's edge meets the stage's, so every
part of it can be brought into the clear. Not drawn by the design: the handoff
draws the stage at Fit only. All of it
is viewport state — none of it touches project content, the playhead, or the
exported framing.

The aspect pod is the thin pod: 34 controls — the aspect dropdown, Crop — on
4 of padding, so it stands 42 tall rather than 60. It floats centred between
the tool pod and the inspector, 24 above the console, and the stage keeps that
much clear below the picture, so at rest the picture never runs under it; the aspect menu opens upward from it. Not drawn by the design: the handoff
hangs the pod from the stage's top left, over the picture.
`SUBTAKE_GALLERY_OPEN=aspect` opens the gallery with the aspect menu showing. After Crop
comes the zoom group: zoom out, the percentage and zoom in on one `sunk` plate,
then Fit as an icon (ArrowsOutSimple). The two steps go a quarter at a time between Fit
and 8x, keeping whatever is at the stage's centre where it is; Fit puts the
picture back and the pan with it. The glyphs are at full strength, as the
captions beside them are, so zoom out and Fit dimming at Fit, and zoom in at
8x, reads as disabled. Everything in the pod is 13: the captions, and the
percentage in the interface face in tabular figures, in a fixed-width slot so
the pod holds its shape while a pinch runs. Not drawn by
the design: the group — the handoff has a single "Fit · 100%" pill. The
console's zoom is the same control. `SUBTAKE_GALLERY_SCREEN=zoom-111` (any
percentage) opens the gallery magnified.

## Motion

Not drawn by the design: every transition below. The handoff draws still
screens; these were chosen on a motion board and live as tokens in
`crates/theme/src/metrics.rs`. All of them honour the Mac's Reduce motion
setting and land in place when it is on.

- A picked inspector panel fades in and rises 6 (`PANEL_ENTER_*`, 180 ms).
  Into a sub-panel (Crop, Background, Shortcuts) it slides in from 24 to the
  right instead, and back out to its parent from the left (`PANEL_DRILL_*`).
- A recorder card opens and closes as the app's menus do, on the same
  timing (`motion::menu_entrance`, `motion::Leave`): it fades in and rises
  `MENU_IN_RISE` (4) up from the bar over `MENU_IN_MS` (140), fades out in
  place over `MENU_OUT_MS` (100) and stays over its control until it has
  gone, and then its window hides. GPUI draws the fade; the frosted
  material under the card is the window server's and takes the same
  opacity and place each frame. Fading the whole window instead let the
  frost lead the card in and trail it out. Opening another card fades the
  open one out over `CARD_SWAP_MS` (60) before the new one comes in; the
  window moves and resizes while nothing shows, so a card never slides,
  grows or shows at the old card's size. A card sets off only once its
  rows are measured, its window is their height and it has been drawn
  there for `CARD_SETTLE_FRAMES` (3), since a resized window's first frames
  reach the screen late and the frost would come in without them. When its
  content changes while it is in view (the source list arriving, an error
  line, the refresh row's spinner) it eases to the new height over
  `RESIZE_MS`, card and frost together: growing, its window takes the new
  height at once and the card rises into it; shrinking, the window holds
  until the card is down. `SUBTAKE_GALLERY_SCREEN=card-sources-cycle`
  empties and refills the Capture source list on a timer. The open card holds
  keyboard focus and hands it back to the bar as it hides. The frost takes
  the app's light or dark theme, not the Mac's. Under Reduce motion every
  fade is a cut. Not drawn by the design: the handoff cross-fades one card
  into the next; with one window for every card, the old fades out before
  the new fades in. `SUBTAKE_GALLERY_SCREEN=card-cycle` opens, swaps and
  closes the cards on a timer.
- The recorder bar keeps its size and place; when it turns from ready to
  counting to recording to writing, its controls fade in (`BAR_SWAP_MS`).
- Pausing eases the clock from red to `sunk` and dims its count to 60%
  (`PAUSED_CLOCK_OPACITY`); Pause and Resume fade in as they swap.
- The Presets dialog fades in rising 12 and fades back out
  (`DIALOG_*`); its frost eases with it, since opacity does not reach a
  backdrop blur.
- A surface whose content changes size while it is open eases to its new
  height over `RESIZE_MS` (200) rather than snapping: the Presets dialog as
  its tab or saved list changes, the command palette as its filter narrows
  the list, a dropdown whose choices change. It opens at its size; only
  changes from there move. `SUBTAKE_GALLERY_SCREEN=presets-cycle` empties
  and refills the Saved list on a timer.
- Dropdowns and the command menu drop 4 as they open and fade out over
  `MENU_OUT_MS` when dismissed. Their entrance settles on a quint, not the
  ease-out every other move uses, so a menu is there the moment it is asked
  for.
- A finished export's tick draws itself on, its label fades in, and the pill
  gives one soft accent pulse (`EXPORT_DONE_*`).
- A zoom step from the aspect pod eases the picture to its new size
  (`PREVIEW_ZOOM_MS`, 220 ms), keeping the stage's centre point still; Fit
  brings the pan home as it shrinks. A second click steps on from the first
  one's end, and a pinch takes over from a step mid-ease.

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
- a timeline region's hover (`REGION_HOVER_INK`) and a held trim handle's
  mark at the full ink;
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
