# Design primitives

Extracted verbatim from the "Stage" redesign handoff
(`Screen Recorder Redesign.zip` → `design_handoff_subtake_editor/`). The
numbers below are the spec, not a summary of it: where this document and a
prose description of the same thing disagree, this document wins.

The 23 Sep implementation review (`docs/IMPLEMENTATION-REVIEW.md`) supersedes
parts of it — the dark recesses, the segmented control, the toggle, the slider
row and the timeline's regions, ruler and playhead. Where the two disagree,
the review wins.

The handoff ships two HTML files. `SubTake Redesign 1a.dc.html` draws six
screens; `Design Spec.dc.html` is the system. Most of the spec is prose in the
markup, but **section 6 renders from a JavaScript array inside the page's own
`<script>` block** — fourteen primitive cards, each with exact geometry, every
state as a delta, and a GPUI builder chain. Opening the file as text, that
section looks empty. It is reproduced here so it stops being invisible.

Two sections give some primitives different numbers. Where that happens both
are recorded and the choice is stated.

---

## 6 · Primitive reference

Fourteen cards, as the handoff gives them. `t` is the theme; `--name` is a
token.

### Button / Primary

*One per surface — Export, Apply.*

| Prop | Value |
| --- | --- |
| height | 44 px |
| padding | 0 22 px |
| radius | full |
| gap | 9 px |
| icon | 17 px |
| font | Geist 500 / 13 |
| fill | `--accent` |
| text | `--on-accent` |
| shadow | `0 8 20 --accent-soft` |

States — hover: fill `--accent-hover` · active: fill `--accent-press`,
`scale .97`, shadow none · focus: ring 3 px `--accent-soft` · disabled:
`opacity .4`, shadow none · loading: label → 16 px spinner, width held.

```rust
div().flex().items_center().gap(px(9.))
  .h(px(44.)).px(px(22.)).rounded_full()
  .bg(t.accent).text_color(t.on_accent)
  .text_size(px(13.)).font_weight(FontWeight::MEDIUM)
  .shadow(vec![BoxShadow { color: t.accent_soft, offset: point(px(0.), px(8.)), blur_radius: px(20.), spread_radius: px(0.) }])
  .hover(|s| s.bg(t.accent_hover))
  .active(|s| s.bg(t.accent_press))
```

### Button / Secondary

*The default button — Presets, Open video.*

| Prop | Value |
| --- | --- |
| height | 44 px |
| padding | 0 18 px |
| radius | full |
| gap | 9 px |
| icon | 17 px |
| font | Geist 400 / 13 |
| fill | `--sunk` |
| text | `--text` |
| shadow | none |

States — hover: fill `--sunk2` · active: fill `--press`, `scale .97` · focus:
ring 3 px `--accent-soft` + inset 1.5 px `--accent` · disabled: `opacity .4` ·
selected: inset 1.5 px `--accent`.

```rust
div().flex().items_center().gap(px(9.))
  .h(px(44.)).px(px(18.)).rounded_full()
  .bg(t.sunk).text_color(t.text).text_size(px(13.))
  .hover(|s| s.bg(t.sunk2))
  .active(|s| s.bg(t.press))
```

### Button / Raised

*A control sitting directly on glass — Suggest zooms, Split, Add.*

| Prop | Value |
| --- | --- |
| height | 40 px |
| padding | 0 16 px |
| radius | full |
| gap | 9 px |
| icon | 16 px |
| font | Geist 400 / 13 |
| fill | `--raise` |
| border | inset 1 px `--raise-line` |
| text | `--text` |

States — hover: fill `white .16`, + `0 2 6` shadow · active: fill `--press`,
`scale .97` · focus: border kept + ring 3 px `--accent-soft` · disabled:
`opacity .4`, border kept.

```rust
div().flex().items_center().gap(px(9.))
  .h(px(40.)).px(px(16.)).rounded_full()
  .bg(t.raise).border_1().border_color(t.raise_line)
  .text_color(t.text).text_size(px(13.))
  .hover(|s| s.bg(t.raise_hover))
```

### Button / Icon

*Every square control — toolbar, pod, close, zoom.*

| Prop | Value |
| --- | --- |
| size | 34 / 40 / 44 px |
| radius | 50% |
| padding | none — centred |
| icon | 14 / 16 / 17 px |
| fill | transparent or `--sunk` |
| text | `--text`, `--muted` when idle |

States — hover: fill `--hover` · active: fill `--press`, `scale .94` · focus:
ring 3 px `--accent-soft` · disabled: `opacity .4` · active tool: fill
`--accent`, text `--on-accent`.

```rust
div().flex().items_center().justify_center()
  .size(px(40.)).rounded_full()
  .text_color(t.muted)
  .hover(|s| s.bg(t.hover))
  .active(|s| s.bg(t.press))
  .when(is_active, |d| d.bg(t.accent).text_color(t.on_accent))
```

### Transport

*Play / pause — the one circular ink button.*

| Prop | Value |
| --- | --- |
| size | 56 px (52 in 1b) |
| radius | 50% |
| icon | **21 px fill weight** |
| fill | `--ink` |
| text | `--on-ink` |
| neighbours | 44 px skip buttons, gap 10 |

States — hover: `opacity .9` · active: `scale .94` · focus: ring 3 px
`--accent-soft` · playing: icon swaps to Pause, no colour change.

```rust
div().flex().items_center().justify_center()
  .size(px(56.)).rounded_full()
  .bg(t.ink).text_color(t.on_ink)
  .active(|s| s.opacity(0.9))
```

### Button / Record

*Starts capture — the only red fill in the app.*

| Prop | Value |
| --- | --- |
| height | 60 px in the bar, 44 px in the titlebar |
| padding | 0 26 px |
| radius | full |
| gap | **11 px** |
| dot | 12 px circle `#ffffff` |
| font | Geist 500 / 15 |
| fill | `--rec` |
| text | `#ffffff` |
| shadow | `0 10 26 rgba(215,50,64,.32)` |

States — hover: brightness 1.1 · active: brightness .92, `scale .97` · focus:
ring 3 px `rgba(215,50,64,.28)` · recording: label → elapsed mono, dot pulses
1 s · disabled: `opacity .4`.

```rust
div().flex().items_center().gap(px(11.))
  .h(px(60.)).px(px(26.)).rounded_full()
  .bg(t.rec).text_color(rgb(0xffffff))
  .text_size(px(15.)).font_weight(FontWeight::MEDIUM)
  .child(div().size(px(12.)).rounded_full().bg(rgb(0xffffff)))
```

### Segmented control

*Two to four exclusive options — Scene / Background, All / Zooms / Captions.*

| Prop | Value |
| --- | --- |
| height | 44 px (40 in toolbars) |
| padding | 4 px track, 0 16 px segment |
| gap | 4 px |
| radius | full, inner full |
| track | `--sunk` |
| font | Geist 400 / 13, 500 when active |

States — active: fill `--ink`, text `--on-ink`, slides 160 ms · inactive: no
fill, text `--muted` · hover: text `--muted` → `--text` · disabled: track
`opacity .4`.

```rust
div().flex().h(px(44.)).p(px(4.)).gap(px(4.))
  .rounded_full().bg(t.sunk)
  .children(opts.map(|o| div().flex_1()
    .flex().items_center().justify_center().rounded_full()
    .when(o.active, |d| d.bg(t.ink).text_color(t.on_ink))
    .when(!o.active, |d| d.text_color(t.muted))))
```

### Toggle

*One boolean — Follow cursor, Connect zooms.*

| Prop | Value |
| --- | --- |
| track | 46 × 28 px, radius full |
| thumb | 22 px circle |
| padding | 3 px |
| travel | 18 px |
| off fill | `--sunk2` |
| on fill | `--accent` |
| thumb | `#ffffff` · `0 1 3 rgba(0,0,0,.3)` |

States — hover: track brightness 1.05 · active: thumb widens to 26 px · focus:
ring 3 px `--accent-soft` · disabled: `opacity .4` · transition: 140 ms
ease-out.

```rust
div().flex().items_center().w(px(46.)).h(px(28.)).p(px(3.))
  .rounded_full().bg(if on { t.accent } else { t.sunk2 })
  .justify(if on { Justify::End } else { Justify::Start })
  .child(div().size(px(22.)).rounded_full().bg(rgb(0xffffff)))
```

### Slider row

*A numeric property — Radius, Scale, Hold.*

| Prop | Value |
| --- | --- |
| height | 44 px |
| padding | 0 18 px |
| radius | full |
| track | `--sunk` |
| fill | `--sunk2`, left-anchored, full height |
| label | Geist 400 / 13 `--text` |
| value | **Geist Mono 400 / 13 `--muted`, right** |

States — hover: track `--sunk2`, fill `--press` · scrubbing: fill
`--accent-soft`, value Geist Mono 500 `--text` · focus: ring 3 px
`--accent-soft` · disabled: `opacity .4` · keys: ← → 1 step, shift 10.

```rust
div().relative().flex().items_center()
  .h(px(44.)).px(px(18.)).rounded_full().bg(t.sunk)
  .child(div().absolute().inset_0().w(relative(frac)).bg(t.sunk2))
  .child(div().flex_1().child(label))
  .child(div().font_family("Geist Mono").text_color(t.muted).child(value))
```

### Menu

*Dropdown, context menu, select popover.*

| Prop | Value |
| --- | --- |
| container | radius 20 px · `--card` · blur 34 sat 1.5 · `--shadow` · inset .5 px `--line` |
| padding | 6 px |
| min width | 180 px |
| item | 34 px tall · 0 12 px · radius 12 · gap 10 |
| icon | 16 px `--muted` |
| shortcut | Geist Mono 11 `--muted`, right |
| separator | 1 px `--line`, 4 px margin |

States — hover: item fill `--hover` · active: item fill `--press`, no scale ·
checked: 16 px accent tick, fill kept · disabled: `opacity .4`, no hover ·
open: `scale .96 → 1` + fade, 120 ms.

> **Conflicts with §11 "Menus and overlays"**, which gives a 32 px item at
> radius 14 with 10 px sides on a 216-wide surface, and makes hover and checked
> both `--sunk`. §11 is the later, dedicated section and is what the code
> follows. See the note in `crates/theme/src/metrics.rs`.

```rust
div().absolute().p(px(6.)).min_w(px(180.))
  .rounded(px(20.)).bg(t.card)
  .shadow(t.shadow).border_p5().border_color(t.line)
  .children(items.map(|i| div().flex().items_center().gap(px(10.))
    .h(px(34.)).px(px(12.)).rounded(px(12.))
    .hover(|s| s.bg(t.hover))))
```

### Tooltip

*Names an icon-only control after 400 ms.*

| Prop | Value |
| --- | --- |
| height | 28 px |
| padding | 0 10 px |
| radius | 10 px |
| fill | `--ink` |
| text | `--on-ink`, Geist 400 / 12 |
| offset | 8 px from the control |
| shortcut | appended at 60% `--on-ink` |

States — delay: 400 ms in, 0 ms out · transition: fade 90 ms · suppressed while
the control is active or a menu is open.

> §11 says the tooltip is **fully round**, not radius 10. §11 wins.

```rust
div().absolute().h(px(28.)).px(px(10.))
  .rounded(px(10.)).bg(t.ink).text_color(t.on_ink)
  .text_size(px(12.))
```

### List row

*A Moments entry, a recent file, a preset.*

| Prop | Value |
| --- | --- |
| padding | 10–12 px 14 px |
| radius | 22 px |
| gap | 14 px |
| thumbnail | 84 × 48 or 96 × 54, radius 12 |
| title | Geist 400 / 13, 500 when selected |
| meta | Geist 400 / 12 `--muted` |
| timecode | Geist Mono 400 / 12 `--muted` |

States — hover: fill `--hover` · selected: fill `--card`, inset 1.5 px
`--accent`, `--shadow`, timecode `--accent` · cut: `opacity .5`, title
strikethrough · dragging: `--shadow`, `scale 1.02`, source at `opacity .3`.

```rust
div().flex().items_center().gap(px(14.))
  .py(px(10.)).px(px(14.)).rounded(px(22.))
  .hover(|s| s.bg(t.hover))
  .when(selected, |d| d.bg(t.card)
    .border_1().border_color(t.accent).shadow(t.shadow))
```

### Timeline region

*A zoom, clip, annotation, caption or audio block.*

| Prop | Value |
| --- | --- |
| height | 30 px (42 on the source lane) |
| radius | 10 px |
| padding | 0 12 px |
| fill | lane hex + `22` |
| border | inset 1 px lane hex + `66` |
| handles | 3 × (h − 12) px, radius 2, inset 3, hex + `cc` |
| label | Geist 400 / 11 `--text`, nowrap, clipped |

States — hover: fill hex + `2e`, handles shown · selected: fill hex + `3d`,
border hex + `ff` · dragging: + `--shadow`, snap lines at `--accent` ·
trimming: grabbed handle goes full hex · disabled: `opacity .5`.

```rust
div().absolute().h(px(30.)).px(px(12.)).rounded(px(10.))
  .bg(lane.hex.alpha(0.13))
  .border_1().border_color(lane.hex.alpha(0.4))
  .when(selected, |d| d.bg(lane.hex.alpha(0.24))
    .border_color(lane.hex))
```

### Panel

*Any floating container — console, inspector, pod, dialog.*

| Prop | Value |
| --- | --- |
| radius | 28 console, inspector, dialog · 30 pod · 40 recorder bar · 26 list plate |
| padding | 18 console and inspector · 20–22 dialog · 8 pod · 10 recorder bar |
| fill | **`--glass` for controls, `--card` for content** |
| blur | 28 pod · 34 console and inspector · 38 recorder bar |
| border | inset .5 px `--line` |
| shadow | `--shadow` |

States — enter: `scale .96 → 1`, fade, 140 ms ease-out · dragging: shadow
doubles, cursor grabbing · window inactive: blur 20, saturate 1.0.

```rust
div().p(px(18.)).rounded(px(28.))
  .bg(t.card).backdrop_blur(px(34.))
  .border_p5().border_color(t.line)
  .shadow(t.shadow)
```

---

## 11 · Menus and overlays

Everything that appears on top of the app. All of them are `--card` — they hold
text you read, and they are dismissible. The geometry is identical in both
themes.

- **Menu surface** — radius 20, `--card`, 6px padding, `--shadow` +
  `inset 0 0 0 .5px var(--line)`. Min width 216, no max.
- **Menu item** — 32 tall, radius 14, 10px side padding, 1px apart. Hover and
  checked both use `--sunk`; the checkmark is what marks state.
- **Checkmark gutter** — fixed 14px column so labels align whether or not an
  item is checked. Accent tick, 14px.
- **Separator** — 1px `--line`, 5px above and below, inset 10 from each edge.
- **Shortcut hint** — Geist Mono 11, `--muted`, right-aligned. Submenus show a
  12px chevron in the same slot.
- **Section header** — 28 tall, the standard 11/600/.1em caps label. Context
  menus name their target here.
- **Destructive item** — label in `--danger`, no fill until hover. Red as a
  fill is `--rec`; red as text is always `--danger`.
- **Tooltip** — 28 tall, fully round, `--ink` on `--on-ink`, 12px, shortcut
  inline at 60% opacity. No arrow.
- **Key cap** — 22 min-square, radius 7, `--sunk` with
  `inset 0 -1.5px 0 var(--line)` for the lip.
- **Dialog** — radius 22, 16px padding, actions right-aligned at 40 tall. Scrim
  is `rgba(10,10,14,.4)` light / `.55` dark, no blur.
- **Toast** — radius 20, 14/16 padding, a 8px status dot, one optional action
  at 30 tall. Never more than one line of text.
- **Progress** — 6px track, radius 3, `--sunk` under `--accent`. Percentage in
  Geist Mono, never inside the bar.
- **Field** — 40 tall, fully round, `--sunk`, 15px padding, 14px leading icon
  at `--muted`. The set has no plain magnifier, so a filter leads with the icon
  of what it filters.
- **Stepper** — 40 tall `--sunk` shell, 6px padding, two 28px round `--raise`
  buttons, value in Geist Mono between them. There is no Minus glyph, so
  steppers pair the two magnifier variants.
- **Checkbox / radio** — 18px, radius 6 for checkbox and round for radio, 10px
  to the label. Checked is a solid accent fill.

---

## 4 · Controls — the size ladder

- **Heights** — 26 chip · 28 toggle · 34 close · 40 toolbar · 44 standard ·
  52 hero · 56 transport · 60 recorder.
- **Horizontal padding** — 12 chip · 16 toolbar · 18 secondary · 22 primary ·
  26 hero. Icon-only controls are square, never padded.
- **Icon-to-label gap** — 9 on buttons, 10 on rows, 8 in pods. Icon size tracks
  control: 16 at 40, 17 at 44, 18–19 at 52–60.
- **Selected** — `inset 0 0 0 1.5px var(--accent)` over `--sunk` or `--card`.
  Never a fill swap.
- **Raised** — `--raise` + `inset 0 0 0 1px var(--raise-line)`, for controls
  that sit directly on glass.
- **Disabled / cut** — `opacity: .5–.55` plus strikethrough on the label.

## 5 · Button states

Six variants — Primary, Secondary, Raised, Ghost, Icon, Destructive — five
states each. Geometry never changes between states; only fill, ring and scale,
so nothing shifts under the pointer.

- **Hover** — tinted variants go one step up the fill scale (`--sunk` →
  `--sunk2`); transparent ones gain `--hover`. Accent goes to `--accent-hover`.
- **Active** — `--press` fill plus `scale(.97)`, or `.94` on round icon
  buttons. Any glow drops to none while held.
- **Focus** — `0 0 0 3px var(--accent-soft)`, with a 1.5px accent inset on
  unfilled variants. Keyboard only.
- **Disabled** — `opacity: .4`, shadows and glows removed, fill kept. Never
  greyed to a different colour.
- **Selected** — not a button state. `inset 0 0 0 1.5px var(--accent)` on rows,
  regions and preset tiles; a full `--ink` fill on segmented controls.
- **Transition** — `background 120ms, transform 90ms, box-shadow 120ms`, all
  ease-out. No transition on disabled.

Non-button states:

- **Menu item** — hover `--hover` at radius 12; active `--press`, no scale;
  checked keeps the fill and adds an accent tick; disabled `opacity: .4` with no
  hover at all. *(§11 supersedes: hover and checked are both `--sunk`.)*
- **List row** — hover `--hover` at radius 22; selected swaps to `--card` +
  accent inset + `--shadow`, and the timecode turns accent.
- **Timeline region** — hover lifts the fill from `22` to `2e`; selected `3d`
  fill with a solid border and both trim handles shown; dragging adds
  `--shadow`.
- **Field and slider** — hover `--sunk2`; focus is the 3px accent ring; while
  scrubbing, the filled portion goes to `--accent-soft` and the value switches
  to Geist Mono 500.
- **Toggle** — off `--sunk2`, on `--accent`; the thumb travels 18px in 140ms
  ease-out and widens to 26px while held.
- **Segmented** — the active segment is an `--ink` pill that slides between
  positions in 160ms; inactive segments hover from `--muted` to `--text` with
  no fill.

## 9 · Timeline

- **Region** — radius 10, fill `hex + 22`, border `hex + 66`; selected goes to
  `3d` / `ff`.
- **Trim handles** — 3 × (lane − 12) rounded 2, inset 3 from each end, at
  `hex + cc`.
- **Playhead** — 2px accent line, full stack height, 16px accent dot with a 4px
  `--accent-soft` ring.
- **Ruler** — 16 tall, Geist Mono 11, ticks at 12.5% intervals.
- **Lane stack** — 42 source, 30 every other lane, 4 between lanes, 78 label
  gutter, 14 gutter-to-track gap.

Lane tints, carried from `src/app/playback.rs` unchanged: Zoom `#397afa`, Clip
`#357c65`, Speed `#dc922d`, Cut `#ee5261`, Annotation `#cbb44f`, Audio
`#a468e9`, Caption `#6396dc`.

## 10 · Elevation

There are no real borders anywhere; every edge is an inset shadow so it never
affects layout.

| Name | Light | Dark |
| --- | --- | --- |
| `--shadow` | `0 24px 60px rgba(20,22,40,.18), 0 2px 6px rgba(20,22,40,.08)` | `0 28px 70px rgba(0,0,0,.5), 0 2px 8px rgba(0,0,0,.3)` |
| Window | `0 44px 100px rgba(0,0,0,.45)` + `inset 0 0 0 .5px --line` | same at `.6` |
| Preview | `0 30px 70px rgba(0,0,0,.35)` | same at `.55` |
| Accent glow | `0 8px 20px --accent-soft` primary, `0 12px 28px` hero | same |
| Hairline | `inset 0 0 0 1px --line` | same |

Only accent and Record buttons glow.

## 12 · Icons

Phosphor Regular on a 256 viewBox, always `fill="currentColor"` so a glyph
takes the colour of whatever holds it. Fill weight is used only for transport —
play, pause, stop, skip.

| Size | Where |
| --- | --- |
| 19px | 52–60 · hero, recorder |
| **18px** | **44–52 · pod, primary** |
| 17px | 44 · standard |
| 16px | 40 · toolbar |
| 14px | menu tick, checkbox |
| **12px** | **caret, disclosure** |

The set is closed — these are the glyphs the app already ships. It has no plain
magnifier, no minus and no right caret; where those are wanted, a CaretDown is
rotated −90° and the magnifier variants stand in.
