# Shared UI primitives

All native screens import app controls from `ui/controls.slint`. That file only re-exports components; the implementations are in `ui/components/`, and shared colour, typography, sizing, radius and state tokens are in `ui/theme.slint`. The editor and recorder use the same implementations.

| Family | Single source | Contract |
| --- | --- | --- |
| Buttons | `ui/components/button.slint` | `ButtonSurface` owns focus, pointer/keyboard activation and disabled behaviour. `ToolButton`, `IconButton` and `RecordButton` compose it. Named variants: secondary, primary, ghost, outline, danger, record. Named sizes: small, compact, regular, large. Active and round are independent states. |
| Dropdowns | `ui/components/dropdown.slint` | Owns both trigger and popup rows. Stable index/value mapping, disabled/empty states, scrollable options, keyboard arrows, Home/End, Enter/Space selection and Escape cancellation. |
| Inputs | `ui/components/input.slint` | Text/numeric value entry, placeholder, focus, disabled/read-only/error states, edit and commit callbacks. |
| Checkboxes | `ui/components/checkbox.slint` | Checked, unchecked, indeterminate and disabled states; pointer, Space and accessibility activation. |
| Switches | `ui/components/toggle.slint` | Controlled checked state, disabled/focus treatment and a change callback. |
| Radio buttons | `ui/components/radio.slint` | Single-choice indicator using shared button interaction; owning group supplies checked state. |
| Preview viewport | `ui/components/preview-viewport.slint` | View-only pinch magnification, anchored image coordinates, scroll panning and Fit reset. |
| Timeline scrubber | `ui/components/timeline-scrubber.slint` | Pointed grey cap, continuous line, shaded six-dot grip and drag target. |
| Filled numeric controls | `ui/components/scrub-field.slint` | Filled slider plus editable number; drag commits on release and text on Enter. |
| Sliders | `ui/components/slider.slint` | Drag preview, release commit, keyboard arrows, bounds and disabled state. |
| Segmented controls | `ui/components/segmented-control.slint` | Shared buttons, option model, current index and selection callback. |
| Selection tiles | `ui/components/choice-tile.slint` | Shared hover/focus/selection and activation for cursor, motion, position, wallpaper and appearance choices. |
| Navigation | `ui/components/rail-button.slint` | Icon button, caption and active rail indicator. |
| Panels and dividers | `ui/components/panel.slint` | Panel, card, popup and overlay surfaces plus separators. |
| Progress | `ui/components/progress.slint` | Bounded progress fill and accessibility value. |
| Labels | `ui/components/label.slint` | Body, muted, heading and caption defaults. |
| Field rows | `ui/components/field-row.slint` | Standard label/control row. |
| Scrolling | `ui/components/scroll-area.slint` | One application boundary around Slint's ScrollView, preserving its scrolling and keyboard behaviour. Scrollbar internals still use the Slint widget style. |

`ui/choices.slint` contains product-level compositions of these primitives. Timeline clips, preview handles, waveform graphics and drag regions remain specialised editor interactions. They are not buttons disguised as rectangles. Native OS menus, system title-bar controls, file pickers and tooltips retain their platform/framework implementations; they are not painted by the app's button/input theme.

To redesign a control, edit its component file. To change the common palette, typography or sizing, edit `theme.slint`. A screen supplies data, actions, layout placement and a named variant; it should not introduce another input, dropdown or generic button implementation. Per-screen content graphics (such as wallpaper colours and thumbnail illustrations) are allowed.

## Gallery and checks

```sh
cargo run --locked --example primitives
cargo run --locked --example primitives -- --smoke
python3 scripts/check-ui-primitives.py
python3 scripts/ui-smoke.py
python3 scripts/launcher-smoke.py
```

The gallery renders light/dark control states and dropdown popups. Its smoke mode injects real Slint pointer/key events to check button activation/disabled behaviour, checkbox Space handling, switches, radios, input commit, dropdown selection/cancellation/empty/disabled states and slider adjustment. Snapshots are saved to `test-output/primitives/`. These are not a complete screen-reader, IME or Windows accessibility acceptance suite.

The boundary check rejects raw standard controls and inline generic button implementations in screen files. It deliberately permits native menus and specialised canvas/timeline pointer handling.

## Transparency and frosted glass

Slint supports alpha backgrounds and transparent windows; the recorder already uses a transparent outer window. Alpha tint alone does not blur what is behind a component.

For desktop blur on macOS, the existing winit backend can request transparent-window blur through `Window::set_blur` / `WindowAttributes::with_blur`. AppKit `NSVisualEffectView` is the platform route for native material integration. These require integration and visual acceptance for our window hierarchy, capture exclusion, rounded edges and accessibility settings. The Mac application now enables transparent-window blur through its winit creation hook. The editor and recorder share translucent material tokens; operating-system blur supplies the desktop backdrop.

The winit 0.30 API documents different platform support: its general blur API is unsupported on Windows and X11, and Wayland requires the KDE blur protocol. Windows needs its own supported backdrop/material integration. A portable opaque fallback remains necessary.

Our pinned Slint 1.17.1 does not expose a general CSS-style backdrop-filter on arbitrary components. Blurring content inside our own window therefore requires additional renderer/compositing work; a blurred drop shadow is not backdrop blur. The timeline grip samples a Gaussian-blurred version of its actual filmstrip artwork, prepared once on the artwork worker. Moving the playhead selects the corresponding portion of that image. This is content-aware filmstrip frosting, not a general-purpose blur of arbitrary overlapping timeline layers.

References: [Slint Rectangle](https://docs.slint.dev/latest/docs/slint/reference/elements/rectangle/), [Slint Window](https://docs.slint.dev/latest/docs/slint/reference/window/window/), [winit blur](https://docs.rs/winit/latest/winit/window/struct.Window.html#method.set_blur), [AppKit visual effects](https://developer.apple.com/documentation/appkit/nsvisualeffectview).

## Comfortable control sizing

`Theme.control-height` is the single 40px height for buttons, icon buttons, dropdown triggers/menu rows, text inputs, filled numeric sliders and plain sliders. The old small/compact/regular/large names are compatibility aliases to that same value. Control widths still follow their content and layout; icon buttons are 40px squares. Compound numeric inputs fill the slider height. Larger choice cards retain their content-driven dimensions. The gallery asserts equal rendered heights, and the boundary checker rejects smaller per-screen height overrides.

Buttons, dropdown triggers, inputs and slider labels share `Theme.control-padding` (12px at the current 40px height), derived from the space around a 16px icon. Plain sliders use a 40px rounded surface, 16px thumb and inset track; their pointer mapping follows the track endpoints. Switches use a 52×30px track with a 24px thumb inside a 40px interaction area, and `FieldRow` shares that height. Numeric slider inputs reserve 70px to preserve value readability with the larger padding.

`ProgressTrack` in `ui/components/progress.slint` owns the reference-style 10px pill track, translucent dark background and soft white gradient fill. `ProgressBar` adds progress accessibility semantics; `FineSlider.progress-style` reuses that visual inside its existing 40px pointer/keyboard control. The timeline position slider uses this variant; zoom and numeric sliders retain their existing styles. This uses translucency, not live backdrop blur.

Filled sliders use a white active surface (translucent white in dark mode), a grey vertical end marker, and a leading icon before their label. Numeric values use a separate inset rounded input. Shared control radius is 16px; endpoint-icon zoom sliders retain the same fill styling and 12px icon padding.

Control geometry is defined by `Theme.control-height` (40px) and `Theme.radius-control` (16px), including buttons previously marked round and slider numeric inputs. The filled slider overlay uses `Theme.slider-fill-inset` (1px) on all edges and `Theme.slider-fill-radius` (outer radius minus inset = 15px); its height is derived as 40 - 2 = 38px.
