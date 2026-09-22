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

## The recorder

A compact rounded floating bar with a source control, audio and webcam options,
a countdown, a red Record control, More and Hide. Option panels expand above it.
The same overlay shows the countdown, the elapsed time in Geist Mono,
pause/resume and stop.

The recorder's windows are borderless and carry no vibrancy material — macOS
gives a borderless window no corner mask, so a blurred view fills the frame
square behind a rounded plate. They therefore use an `overlay` tone of their own
rather than `glass`, and take their shadow from the window server or not at all.
This is the one place the palette departs from the handoff, and `crates/theme`
says so where the token is defined.

## Gestures

Pinch over the timeline zooms between 1x and 100x, anchoring the time under the
pointer. Pinch over the stage magnifies from Fit to 8x, anchoring the image
point under the gesture; two-finger scroll pans, and Fit resets both. All of it
is viewport state — none of it touches project content, the playhead, or the
exported framing.
