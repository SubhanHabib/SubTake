# Shared UI primitives

Every native screen is built from `crates/ui`. Colour, typography, sizing,
radius and state tokens are in `crates/theme`. The editor and the recorder use
the same implementations.

Boundary check: `python3 scripts/check-ui-primitives.py`. It requires the theme
and control crates, the runtime/state adapters and the editor surface; it
rejects a control implementation or a layout factory outside `crates/ui`, a
palette outside `crates/theme`, a shared crate reaching into application state,
and a bare numeric literal where a type or icon token belongs. A pass is a
static architecture result only — it certifies no rendering and no input.

## The kit

One file per control under `crates/ui/src/controls/`, re-exported by
`crates/ui/src/controls.rs` so call sites write `subtake_ui::button(..)` and
never name the file.

| Family | File | Contract |
| --- | --- | --- |
| Buttons | `button.rs` | `Button` owns every variant — primary, secondary, raised, ghost, danger, record, transport — plus the icon-only form and `tool_button`, the tool pod's entry, which takes the accent fill outright while its panel is open. Also focus, hover tweening, the glow under a filled plate and the accent inset a selected control carries. `focus_ring` and `hairline` live here because every other control's edge is built from them. |
| Dropdowns | `dropdown.rs` | Trigger plus anchored popup. Stable index/value mapping, keyboard arrows, Home/End, Enter/Space, Escape, and a menu that occludes what is behind it. |
| Menus | `menu.rs` | The plate, the scrolling list, the row and the separator every transient menu is built from. A row is **not** a `Button`: a menu's current item is marked by an accent tick in a fixed gutter over a `sunk` fill, never by an accent pill. |
| Inputs | `input.rs` | `TextInput`: value entry, placeholder, focus, disabled, edit and commit callbacks. |
| Sliders | `slider.rs` | The scrub field — a filled slider that *is* the row, with its caption inside the plate and its value in Geist Mono on the right. Track `sunk`, hover `sunk2`, scrubbing fill `accent_soft`. Drag previews, release commits, and the committed number is quantised to the step the display shows. |
| Switches | `switch.rs` | `toggle` and `switch`: `sunk2` off, `switch_on` on, a white thumb in both states travelling 18 in 140ms. Not wired: the thumb widening to 26 while held. |
| Segmented controls | `segmented_control.rs` | A raised `seg_active` pill with a faint shadow and hairline, sliding between positions; the active label is `text` at 500, inactive labels hover from `muted` to `text` with no fill. |
| Panels | `panel.rs` | `Surface` — `Panel` (glass, holds controls), `Content` (card, holds text), `Pod` (glass, holds icons), `Card`, `Popup`, `Overlay` — plus each one's radius and blur, the caps labels and the dividers drawn on them. A float takes its blur by being wrapped in `frosted` at its own surface's radius and blur; `panel_variant` only paints the plate. |
| Tiles | `tile.rs` | Colour swatches, captioned thumbnails, selection cards and the empty-state plane. |
| Field rows | `field_row.rs` | The label/control row, the group card, the setting card and the tile grid. |
| Status | `status.rs` | The unsaved dot, the progress rule, the context chip and the composer footer. |
| Tooltips | `tooltip.rs` | A 28-tall `ink` pill. Only a control whose caption cannot be read gets one. |

Shared, outside `controls/`: `layout.rs` (`row`, `column`, `measure`),
`icon.rs`, `typography.rs` (`mono`, `title` and the two families nothing else
names), `frost.rs` (the backdrop-blurred float, the nested scene layer and the
scroll-edge fade), `motion.rs` (the tween store) and `fonts.rs`.

`crates/ui/src/unused/` holds the ten primitives the handoff specifies that the
app has no call site for. Nothing calls them; the module is public so the
compiler does not warn about that. Their measurements are module-local consts
rather than `Theme` tokens — promoting one means moving its numbers into
`metrics.rs` along with it.

Timeline regions, preview handles, waveform graphics and drag regions stay
specialised editor interactions. They are not buttons disguised as rectangles.
Native OS menus, system title-bar controls and file pickers keep their platform
implementations.

To redesign a control, edit its file. To change a colour, size or gap, edit
`crates/theme`. A screen supplies data, actions, layout placement and a named
variant; it does not introduce another input, dropdown or generic button.

## Frosted glass

gpui at the pinned revision (the `zeronsh/zui` fork) carries destination alpha
on transparent windows, and the editor lays one `WindowGlass` under gpui's view
(`native/WindowGlass.swift`), so the window itself is a real material at the
spec's `--bg` blur and saturation (`Theme::WINDOW_BLUR`, `WINDOW_SATURATION`). Blur *inside* the window comes from
`frost::frosted`, which paints a backdrop blur and then the whole subtree inside
one scene layer.

The single layer is the point. With per-primitive ordering, a hover repaint
elsewhere could reassign a card's quads below its own blur, and washes, dividers
and edges intermittently got snapshotted and blurred away. Inside one layer the
relationship is structural: blur, then shadow, tint, edge, rows, text.

`frost::layered` restores stacking for an overlay *inside* a frosted card, where
one shared draw order otherwise groups by primitive kind and puts a close
button's circle under the thumbnail it sits on.

`frost::fade_edges` fades a scroll region across 16px at its top and bottom.
A scroll region over glass cannot hide its clip line behind a gradient scrim —
there is no paintable colour equal to "the blurred desktop behind this window" —
so the fork's `Window::with_edge_fade` multiplies each primitive's alpha by a
ramp instead, which composites correctly over anything. Pair it with `FADE_BAND`
of padding inside the content so a region that fits is never dimmed.

The handoff pairs each blur with a `saturate()`. gpui has no filter for it, so
floats carry only the blur; the window's own material carries both.

## Running the gallery

```sh
python3 scripts/dev.py --gallery        # every control, fixture data, no project
python3 scripts/dev.py --gallery light  # the same in the light appearance
python3 scripts/dev.py --once           # build and launch the app, no watching
python3 scripts/check-ui-primitives.py
```

The gallery is live: toggles flip, dropdowns pick, sliders move and the playhead
drags. It is the fastest way to see a control in every state at once.
