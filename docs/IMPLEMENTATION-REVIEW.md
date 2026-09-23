<!--
  The 23 Sep implementation review, as the handoff gives it
  (`Screen Recorder Redesign (2).zip` → `Fixes - implementation review.md`).
  It wins over docs/DESIGN-PRIMITIVES.md wherever the two disagree.

  What SubTake takes from it:
  - Fixes 1–11 and the token changes, as written.
  - Not the *Layout* section or card 12: the aspect pod stays at the
    stage's bottom centre, Help stays in the tool pod, and the pods keep
    their own top lines.
  - Not the zoom drawn in section A or card 9: the stage and the console
    share one zoom control, with Fit as an icon.
  - Fix 3's fill clipped by the track's round end is drawn as a
    track-sized pill cut to the level's width, which needs no rounded clip.

  Superseded by Timeline 3a (`Screen Recorder Redesign (3).zip` →
  `Timeline 3a.md`), which docs/UI-DESIGN.md describes as built:
  - Fix 4's playhead chip on the line and hidden ticks: the bubble sits
    above the ruler, and no tick is hidden.
  - Fix 6's 16px strip for an empty lane: an empty lane is not drawn.
  - Fixes 7–8's `lane_track` behind the lanes: there is no lane track.
  - The region kind icons and `2× Speed` tooltip below: every region has
    a plate, and a speed region is labelled `2×`.
-->

# Handoff: SubTake implementation fixes (review of 23 Sep)

## Overview

This is a set of eleven fixes to the SubTake editor and recorder as currently built. They came from reviewing the running app against the design. Each one corrects a mismatch or a usability problem that only showed up once the design ran on real content.

The visual reference is `Implementation Review.dc.html` in this folder. Section A shows the editor with every fix applied, in light and dark. Section B shows each fix as a before and after pair. The **Before** side of each pair reproduces what the app currently renders, and the **After** side is the target. Card numbers in that file match the fix numbers below. Card 12 is a layout sketch; its sizing changes are covered here under fixes 3, 6 and 11 plus the *Layout* section.

## About the design files

The files here are **design references created in HTML**. They show intended look and behavior; they are not code to copy. SubTake is a Rust app built on **GPUI**, so implement these in the existing codebase and its patterns: the token struct in `crates/theme/src/lib.rs`, the primitives already built from `Design Spec.dc.html`, and the Phosphor icons in `assets/icons/`.

## Fidelity

**High fidelity.** Every value below is exact. Where this document and an earlier spec disagree, this document wins. The *Supersedes* section lists every conflict.

---

## Token changes

Four tokens are new and two dark values change. Add them to the theme struct with both variants.

| Token | Light | Dark | Used by |
| --- | --- | --- | --- |
| `switch_on` **new** | `#5b5d65` | `#6e707a` | Toggle track when on (fix 1–2) |
| `seg_active` **new** | `#ffffff` | `rgba(255,255,255,0.14)` | Active segment fill (fix 1–2) |
| `slider_fill` **new** | `rgba(20,20,26,0.06)` | `rgba(255,255,255,0.07)` | Filled part of a slider row (fix 3) |
| `lane_track` **new** | `rgba(20,20,26,0.07)` | `rgba(255,255,255,0.05)` | Timeline lane background (fix 7–8) |
| `sunk` — dark only | unchanged | `rgba(255,255,255,0.05)` (was `rgba(22,22,27,0.62)`) | All recesses |
| `sunk2` — dark only | unchanged | `rgba(255,255,255,0.10)` (was `rgba(255,255,255,0.14)`) | Hover / filled recesses |

**Why the dark recess change.** Dark `sunk` was darker than `card`, so recesses on a card (segmented tracks, slider rows, toggle rows) were almost invisible. In dark, recesses now lighten instead of darken. This affects every `sunk` surface in dark mode, not only the Scene panel. After changing it, check the recorder cards, the console buttons and the title pill in dark.

---

## Fix 1–2 — Segmented control and toggle (Scene panel and everywhere else)

**Problem.** The active segment was a solid `ink` pill: black in light, white in dark. It was the heaviest mark in the panel, heavier than Export. The toggle's on-state was also `ink`. In dark that gave a white track with a black thumb, which many people read as "off".

### Segmented control

| Part | Value |
| --- | --- |
| Track | 44 tall, radius full, `sunk`, padding 4, gap 4 |
| Segment | flex 1, radius full, centred label, Geist 400 / 13 |
| Active segment | fill `seg_active`; shadow `0 1px 3px rgba(20,22,40,0.12)` plus a 0.5px `line` inset; label `text`, weight 500 |
| Inactive segment | no fill, label `muted` |
| Hover (inactive) | label goes `muted` → `text`, no fill |
| Change | the active pill slides between positions over 160ms ease-out |
| Disabled | track `opacity 0.4` |

### Toggle

| Part | Value |
| --- | --- |
| Track | 46 × 28, radius full, padding 3 |
| Thumb | 22 circle, **always `#ffffff`** in both themes, shadow `0 1px 3px rgba(0,0,0,0.3)` |
| On | track `switch_on`, thumb right |
| Off | track `sunk2`, thumb left |
| Travel | 18px over 140ms ease-out; while pressed the thumb widens to 26px |
| Focus | 3px `accent_soft` ring |
| Disabled | `opacity 0.4` |

Accent is not used for either control. On/off is shown by the plate and the thumb position.

**Done when:** in both themes, the Scene panel's heaviest element is the title rather than a control; a toggle is identifiable as on or off without reading its label; no toggle anywhere has a dark thumb.

---

## Fix 3 — Slider rows

**Problem.** The fill was a white, fully rounded shape. At low values (Radius 4px) it shrank to a blob that read as a knob. A vertical tick at the fill edge cut through the label ("Padd|ing").

| Part | Value |
| --- | --- |
| Track | 44 tall, radius full, `sunk`, padding 0 18, **clips its children** |
| Fill | left-anchored, full height, width = value %, fill `slider_fill`, **no radius of its own**. The track's clip rounds the left end; the right edge stays square. |
| Label | Geist 400 / 13 `text`, left |
| Value | Geist Mono 400 / 13 `muted`, right |
| Tick at fill edge | **removed** |

States:

| State | Change |
| --- | --- |
| Idle | no thumb |
| Hover | track `sunk2`; thumb appears — 4px wide, radius 2, inset 12px top and bottom (20 tall), `muted`, centred on the fill edge |
| Dragging | thumb inset 10px top and bottom (24 tall), `text`; value switches to Geist Mono 500 `text` |
| Focus | 3px `accent_soft` ring; ← → change by 1 step, shift ×10 |
| Disabled | `opacity 0.4` |

**Done when:** at 0–5% the row shows no blob, only a sliver of fill; no line crosses the label at any value; the thumb is visible only on hover and during drag.

---

## Fix 4 — Playhead and ruler

**Problem.** The playhead's triangle head covered the ruler label under it ("37.0s" rendered as "7.0s"). The ruler counted seconds (18.5s, 37.0s…) while the transport read `00:37.15 / 02:28`.

### Ruler

- Labels use `m:ss` — `0:00`, `0:15`, `1:30` — in Geist Mono 400 / 11 `muted`, row height 16.
- Tick interval: the smallest value from `1, 2, 5, 10, 15, 30, 60, 120, 300` seconds that keeps labels at least **80px** apart at the current zoom. At the default window (≈1376px track, 2:28 duration) that is 15s.
- Recompute the interval when the timeline zoom changes.

### Playhead

- Line: 2px wide, `accent`, radius 2, full height of the ruler plus lane stack.
- **The triangle head is removed.** In its place a timecode chip is centred on the line at `top: -2px`:
  - 20 tall, padding 0 8, radius full
  - fill `accent`, text `on_accent`, Geist Mono 500 / 11
  - format `m:ss.cc`, e.g. `0:37.15`
- **Hide any tick label whose box comes within 4px of the chip's box.** Show it again once the playhead moves away. Do not fade it; toggle it.

**Done when:** at any playhead position no tick label is covered or clipped; ruler and transport use the same time format.

---

## Fix 5 — Short regions

**Problem.** Region labels truncated to "2× Sp…", "Click…", "An…", and narrow caption regions showed nothing at all.

- Measure the region's **rendered width**. Below **48px**, show only the kind's icon, 13px, centred, in that lane's ink color (see fix 7–8). At 48px and above, show an optional 13px icon, a 6px gap, then the label, ellipsised.
- Kind icons (Phosphor Regular): Speed → `Timer`, Cut → `Scissors`, Annotation / Text → `TextT`, Caption → `ClosedCaptioning`. Zoom, Clip and Audio have no icon and show their label only.
- **Tooltip** on hover after 400ms, whenever a region is icon-only **or** its label is truncated:
  - 28 tall, padding 0 10, radius 10, fill `ink`, text `on_ink` Geist 400 / 12
  - content: full label, then the range in Geist Mono at 60% opacity — e.g. `2× Speed  0:26–0:34`
  - 8px above the region, centred on it; 90ms fade; suppressed while dragging

**Done when:** no region shows an ellipsis fragment shorter than four characters; every region can be identified by hover.

---

## Fix 6 — Empty lanes collapse

**Problem.** Every lane took 30px whether or not it held anything, which pushed the stage down.

| State | Lane track | Gutter label |
| --- | --- | --- |
| Has regions | 30 tall, radius 12, `lane_track` | Geist 400 / 12 `muted`, row 30 |
| **Empty** | **16 tall, radius 8, no fill, 1px `line` inset** | Geist 400 / **11** `muted`, row 16 |
| Empty + hover or drag-over | expands to 30 over 140ms ease-out; fill `hover`, 1px `line` inset; placeholder "Click or drag to add a {kind} region" in Geist 400 / 11 `muted`, padding 0 12 | row grows to 30 alongside |

- Hover counts over either the gutter label or the track.
- Clicking the expanded lane adds a default region of that kind at the playhead.
- The lane collapses back 300ms after the pointer leaves, unless a region was added.
- The Source lane never collapses.
- Gap between lanes stays 4px in every state.

**Done when:** a project with only zooms and a voice-over shows those two lanes at full height and every other lane as a 16px strip.

---

## Fix 7–8 — Lane contrast

**Problem.** In light, the pastel regions sat almost on the grey track. In dark, lanes had no track at all, so regions floated.

- Lane track: `lane_track` (new token above), 30 tall, radius 12.
- Region fills and ink are derived from each lane's existing hue:
  - **Light:** fill `hsl(h, 75%, 82%)`, ink `hsl(h, 65%, 25%)` (previously 87% / 28%)
  - **Dark:** fill `hsl(h, 35%, 34%)`, ink `hsl(h, 60%, 88%)` (previously 30% / 85%)
- Region: radius 10, padding 0 10, label Geist 500 / 11 in the ink color, no border.
- Selected region: add `inset 0 0 0 1.5px accent`. Nothing else changes.
- Audio: the full-width region carries a waveform in the ink color at alpha `0x55`, inset 6px top and bottom.

Computed values:

| Lane | Source hex | Hue | Light fill | Light ink | Dark fill | Dark ink |
| --- | --- | --- | --- | --- | --- | --- |
| Zoom | `#397afa` | 220° | `#afc6f4` | `#163269` | `#384d75` | `#cedaf3` |
| Clip | `#357c65` | 161° | `#aff4dd` | `#16694e` | `#387561` | `#cef3e7` |
| Speed | `#dc922d` | 35° | `#f4d6af` | `#694616` | `#755b38` | `#f3e3ce` |
| Cut | `#ee5261` | 354° | `#f4afb5` | `#69161e` | `#75383e` | `#f3ced2` |
| Annotation | `#cbb44f` | 49° | `#f4e7af` | `#695a16` | `#756a38` | `#f3ecce` |
| Audio | `#a468e9` | 268° | `#cfaff4` | `#3d1669` | `#553875` | `#dfcef3` |
| Caption | `#6396dc` | 215° | `#afccf4` | `#163969` | `#385275` | `#cedef3` |

**Done when:** in both themes every region is clearly separate from its track at a glance, and region labels clear 4.5:1 against their fill.

---

## Fix 9 — Snap toggle

**Problem.** The Magnet button was filled with `accent`. Snap is an on/off state, and accent is reserved for the playhead, selection, the active tool and the primary action.

| State | Value |
| --- | --- |
| Snap on | 40 round, fill `sunk`, glyph `text` |
| Snap off | 40 round, no fill, glyph `muted` |
| Hover | on: `sunk2`; off: `hover` |
| Active | `press`, scale 0.94 |

**Done when:** no toggle-type control in the console uses accent.

---

## Fix 10 — Recorder bar

**Problem.** Mic and camera both sat on a plate whether they were on or off, so you could not tell which was live. The drag handle, More and Close controls were faint.

| Control | On | Off |
| --- | --- | --- |
| Microphone | 60 round, `sunk` plate, `Microphone` glyph in `text` | 60 round, **no plate**, `MicrophoneSlash` glyph in `muted` |
| Camera | 60 round, `sunk` plate, `VideoCamera` glyph in `text` | 60 round, **no plate**, `VideoCameraSlash` glyph in `muted` |

- Drag handle (`DotsSixVertical`, 18), More (`DotsThree`, 20) and Close (`X`, 17) go to `text` at full opacity. Their sizes are unchanged.
- The display pill, countdown pill and Record button are unchanged.
- The same plate-and-glyph rule applies in the recording and paused states from round 2.

**Done when:** a screenshot of the bar shows which inputs are live without hovering anything.

---

## Fix 11 — Titlebar and status line

**Problem.** In dark, Record and Presets had no plate and read as text labels. The gallery-mode status took a whole console row plus a divider.

- **Record:** 44 tall pill, padding 0 18, fill `sunk`, `Record` glyph 17 + "Record", in **both** themes.
- **Presets:** 44 round, fill `sunk`, `Stack` glyph 17, in both themes.
- Export is unchanged.
- **Title pill status chip:** when the document has a status, append a chip inside the title pill after the name:
  - 26 tall, padding 0 10, radius full, fill `sunk2`, Geist 400 / 11 `muted`
  - the title pill's right padding drops from 16 to 6 while a chip is present
  - gallery mode shows "Gallery mode"; use the same slot for any future document-level status
- **Remove** the status row at the bottom of the console and its divider.

**Done when:** both titlebar buttons read as buttons in dark; the console has no status row.

---

## Layout

These come from the sketch in card 12 and should land together with the fixes above.

- **Top alignment.** The tool pod, the aspect pod and the inspector share one top edge — the first pixel below the 64px titlebar.
- **Aspect pod.** Sits above the preview, 12px gap, left-aligned to the preview's left edge. It no longer overlaps the video.
- **Inspector height.** The inspector runs from that top edge down to the console's top edge. It does not stop at the stage bottom. At 1600 × 1000 that is 466px.
- **Lane height.** Lanes are 30px (Source 42), per spec. The build currently renders about 46.
- **Help** (`?`) leaves the tool pod and moves to the Help menu. Gear stays alone below the pod's divider.

At 1600 × 1000, the result is a preview of 729 × 410 with the inspector fully visible down to its Background section.

---

## Supersedes

| Earlier source | What changes |
| --- | --- |
| `Round 2 - batch 1.md`, *Spec changes* | Toggle on-state was `ink` track with `on_ink` thumb. It is now `switch_on` with a white thumb. |
| `Design Spec.dc.html` §1 tokens | Dark `sunk` and `sunk2` values; four tokens added. |
| `Design Spec.dc.html` §5–6, segmented | Active segment was an `ink` fill. It is now `seg_active` with a shadow. |
| `Design Spec.dc.html` §6, slider row | Fill was `sunk2`; it is now `slider_fill`, square right edge, with a hover/drag thumb. |
| `Design Spec.dc.html` §9, timeline | Regions were `hex + 22` translucent. They are now opaque derived fills (table above). The playhead's round dot is replaced by the timecode chip. Ruler labels are `m:ss`. |
| `README.md`, *Interactions* | Snap moves off accent. |

## Files

| File | Use |
| --- | --- |
| `Implementation Review.dc.html` | Visual reference. Cards 1–11 match fixes 1–11; card 12 is the layout sketch; section A is the full target. |
| `Design Spec.dc.html`, `README.md`, `Round 2 - batch 1.md` | Earlier specs. Read them together with *Supersedes* above. |
