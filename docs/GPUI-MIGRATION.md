# GPUI migration verification checklist

Status: GPUI migration implemented. Fresh local validation is recorded below; unchecked acceptance gates remain explicit. Earlier Slint screenshots, timings and JSON validation reports remain historical evidence for their recorded revisions and do not certify GPUI.

## Fresh local evidence — 21 September 2026

- All-target test run: **68 passed, 1 ignored**, including completion-queue wake and preview pan geometry regressions. The ignored test is not a pass.
- Packaged UI smoke: **11 cases passed**, including Vision OCR of at least three distinct visible editor labels per case. [gpui-ui-validation.json](gpui-ui-validation.json) records the binary hash, cases and OCR evidence. These automated cases check callbacks, snapshots and visible text; they do not inject mouse events.
- Separate real mouse/keyboard validation, reported by the main migration task: opening the Cursor panel, committing a numeric value with Enter, seeking and undo all verified. This evidence is separate from callback smoke tests and does not cover every control or gesture.
- OCR guard validation: the missing-font screenshot `test-output/gpui-first.png` fails; `test-output/gpui-fonts.png` passes. Preview fixture text, including “Native SubTake”, does not count. A pass does not certify rail truncation or complete visual parity.
- Optimized local package: `python3 scripts/build-mac.py --release` and `codesign --verify --deep --strict dist/SubTake.app` passed. All 103 icon/branding assets were verified. `test-output/gpui-packaged-dependencies.txt` records every packaged executable/dylib's `otool -L` output, with no Homebrew-prefix dependencies remaining.
- Recorder: all six configuration/lifecycle cases and the generated-window capture passed. [gpui-launcher-validation.json](gpui-launcher-validation.json) records countdown cancellation, pause/resume/stop, editor handoff and independent frame-content checks that exclude the red occluder. Microphone, webcam and system audio were disabled for this capture.
- Packaged media smoke: `scripts/smoke.py` passed with the bundled binary, FFmpeg and FFprobe. It verifies 90 MP4 frames / 3 seconds, 30 GIF frames, mixed audio amplitudes, captions and SRT/VTT, trim/speed/zoom, webcam composition, backgrounds and five cursor families. Evidence: `test-output/smoke-results.json`.
- Dark editor: `test-output/gpui-editor-dark-final.png` was visually inspected and passed the label OCR guard. The light minimum-size screenshot was also visually inspected. Neither is a claim of exhaustive visual parity.

Environment: Apple Silicon (`arm64`), macOS 27.0, base revision `931f2156` plus the local migration changes. Report JSON files identify the tested binary by SHA-256; this is an uncommitted local implementation, not a published release.

This records local implementation evidence, not physical trackpad, accessibility, complete capture-device, performance or public-release certification.

Known accessibility limitation: desktop inspection exposed the native window and menus, but not a rich accessibility tree for the custom GPUI controls. Do not claim VoiceOver parity with Slint. Keyboard focus/activation and text input are implemented, but comprehensive IME and assistive-technology acceptance remains outstanding.

## Ownership and runtime

The core editor and recorder use GPUI surfaces with shared tokens in `crates/theme` and controls in `crates/ui`. `src/ui_state.rs` preserves the EditorWindow get/set/on/invoke model contract; `src/ui_runtime.rs` provides application lifecycle, timers and image adaptation. Preserving a callback API does not prove that a rendered control invokes it. The optional HyperFrames workspace intentionally retains WKWebView in `scripts/agent-workspace.m`.

The development supervisor watches `src/`, `crates/theme/`, `crates/ui/`, `scripts/`, `assets/`, and root Cargo/build files. Rust, crate manifests, Metal sources, fonts and image assets trigger rebuilds. It compiles the application binary before requesting a normal quit, retains the current app on build failure, and respects recording/busy state and unsaved-document cancellation. `--once` still supervises the launched child but skips source-triggered rebuilds.

Packaging explicitly builds `subtake-native`, compiles the Swift helpers, copies media resources, relocates Homebrew dynamic libraries and signs the staged bundle before replacing the existing package. GPUI does not remove the existing FFmpeg, cursor, Whisper or capture-helper resource requirements. Verify any additional GPUI runtime resources against the actual pinned dependency build; do not assume a successful Cargo build proves standalone packaging.

## Repeatable checks

### Local build toolchain

GPUI is pinned to 0.2.2 with default features disabled and both `runtime_shaders` and `font-kit` explicitly enabled. On macOS, omitting `font-kit` selects a no-op text system: the app can compile and draw controls while every label is invisible. The UI uses `.SystemUIFont`; the OCR smoke guard catches this regression. Runtime shaders avoid requiring the separately installed offline Metal compiler.

The runtime processes bounded completion batches and re-arms its wake when work remains. Worker threads carry a window identity, never an `Rc` to UI state. Native recorder style/position changes are deferred outside GPUI application borrows. Preview image uploads explicitly convert RGBA to GPUI's BGRA byte layout.

The migration build log showed that the exact Skia 0.99.0 prebuilt archive returned HTTP 404. rust-skia then configured a source build but could not execute Ninja. This is separate from GPUI's missing offline Metal compiler; the main migration enables GPUI `runtime_shaders`. Do not override Skia's binary URL with an archive from another feature set or version.

After installing Ninja, the source build completed but its generated Rust bindings failed with E0425 on `_Traits` in `std___hash_table___node_allocator`. Inspection of the old Slint renderer manifest and successful cached build outputs showed that Slint had implicitly enabled Skia's `gl` feature. The root `skia-safe` declaration now explicitly retains `gl` alongside `textlayout` and `svg` (with `metal` on macOS), restoring the previous `gl-jpegd-jpege-metal-pdf-svg-textlayout` archive selection. No generated bindings or registry sources were patched. `python3 scripts/build_env.py check --locked -p skia-safe` passed after this change. This is dependency-level validation only; the full migrated app and runtime acceptance gates remain separate. Enabling the feature does not switch the application's compositor from Metal to OpenGL.

The checkout-local environment is reproducible without sudo or global packages:

```sh
python3 -m venv target/build-tools
target/build-tools/bin/python -m pip install ninja==1.13.0 cmake==3.31.6
target/build-tools/bin/ninja --version
target/build-tools/bin/cmake --version
python3 scripts/build_env.py check --locked --all-targets
```

`scripts/build_env.py` prepends `target/build-tools/bin` to the child process PATH and sets `SKIA_NINJA_COMMAND` to the discovered Ninja unless an explicit override already exists. It does not change the parent shell or Cargo configuration. Both dev and packaging scripts use this environment automatically. Direct `cargo` calls require equivalent PATH setup or the wrapper. Removing `target/` also removes these tools. CMake is available for native dependency builds; the observed Skia failure specifically required Ninja. Ninja and CMake version commands were executed successfully; this alone does not establish that the full Skia/application build passes.

Use one root-workspace Cargo build at a time. The wrapper does not acquire a separate lock or start background builds; coordinate with the active migration build rather than launching duplicate checks.

### Boundary and evidence scripts

```sh
python3 scripts/check-ui-primitives.py
```

This is now a static GPUI architecture check. It requires the actual editor surface/runtime/state and shared crate sources, checks the surface consumes shared UI/theme crates, rejects retired Slint dependency/build references, and confines generic control and theme definitions to their owning crates. Shared presentation crates may depend on GPUI (and UI on theme), not application state. The current dependency allowlist deliberately requires review when a shared crate adds a dependency. These source-pattern checks do not prove event wiring, palette usage at every call site, accessibility or rendering. A missing GPUI surface is an error, not an empty successful scan.

`scripts/ui-smoke.py` requires the new controller-callback smoke marker, a freshly generated nonempty snapshot and successful Vision OCR. It writes `gpui-*` artifacts and `docs/gpui-ui-validation.json`, explicitly excluding pointer/keyboard/gesture coverage. `scripts/launcher-smoke.py` writes artifacts under `test-output/gpui-launcher/` and its report to `docs/gpui-launcher-validation.json`, preserving historical launcher outputs. No launcher run is implied by this path change. `scripts/validation-report.py` refuses its old aggregate workflow when the GPUI runtime exists, because its mixed historical inputs cannot certify a new backend. Historical JSON reports are retained; launcher results must not be interpreted as editor pointer coverage.

Commands for repeating validation:

```sh
python3 scripts/build_env.py check --locked --all-targets
python3 scripts/build_env.py test --locked --all-targets
python3 scripts/build_env.py test --locked --example timeline_interaction
python3 scripts/build_env.py run --locked --example primitives -- --smoke
python3 scripts/build_env.py run --locked --example timeline_interaction
python3 scripts/dev.py --once
python3 scripts/build-mac.py --release
codesign --verify --deep --strict dist/SubTake.app
```

Run GUI commands one at a time. The `primitives` name is retained for command compatibility; it now opens the real editor surface and checks preview callback delivery. It is not the former exhaustive primitive gallery. `timeline_interaction` checks source/output mapping across a trimmed interval, seek callback ordering and playhead updates, selection payload/extend propagation, and seek/selection isolation. Neither example injects pointer events or claims gesture coverage. Their new success markers deliberately differ from the old Slint reports. The old `SUBTAKE_PREVIEW_GESTURE_SNAPSHOT` example option and primitive snapshot outputs are no longer provided.

## Acceptance gates — completed checks and remaining scope

- [x] All-target tests: 68 passed, 1 ignored.
- [x] Eleven packaged UI cases, fresh screenshots and visible-label OCR.
- [x] Focused real input checks: Cursor panel mouse activation, numeric Enter commit, seeking and undo.

- [x] Dependency audit: no Slint or `i-slint-core` in `cargo tree --locked` (`test-output/gpui-dependency-tree.txt`), build script, examples or production Rust UI. Retired `.slint` source files removed; shared presentation boundary check passes for 38 Rust sources.
- [x] Both rebuilt examples launched and exited successfully: `target/debug/examples/primitives --smoke` and `target/debug/examples/timeline_interaction`. Their markers certify surface/model callbacks, not pointer gestures.
- [ ] Watcher: changes to both shared crates, their Cargo manifests, GPUI views, fonts and images rebuild; a syntax error preserves the old app; recovery relaunches once; `--once` does not rebuild.
- [ ] Lifecycle: ordinary quit, cancelled unsaved-document prompt, recording/export deferral, tray Open/Quit, editor close/reopen and timer cancellation all work without duplicate processes.
- [ ] Remaining controls: broader mouse/keyboard activation, focus order/rings, disabled controls, slider release commit, dropdown navigation/cancellation, IME and accessibility. Focused Cursor/numeric/undo checks above do not close this gate.
- [ ] Remaining timeline interactions beyond verified seeking: ruler/filmstrip scrub on press, movement and release; capture beyond bounds; playhead hit priority over regions; move/resize/group selection; source-clock consistency.
- [ ] Gestures: physical trackpad or supported GPUI event tests for timeline/preview pinch, anchor preservation, zoom limits, cancellation, pan and Fit; preview gestures leave timeline/playhead/document state unchanged.
- [ ] Visuals: fresh light/dark editor and recorder captures at 1360×880 and 980×680; controls, text, menus, preview images, filmstrip/waveform and native material boundaries. Preserve approved Porcelain identity.
- [ ] Images: runtime Image decoding, empty/loading/failure states, preview replacement and memory lifetime; no stale frame after media/project changes.
- [ ] Package: inspect every Mach-O dependency with `otool -L`; launch with a restricted PATH, open a fixture, play audio/video, export, and exercise recorder handoff using bundled resources.
- [ ] Capture permissions: real screen, microphone, system audio and camera combinations, pause/resume, cancellation, failure recovery and long-session synchronization.
- [ ] Performance: measure startup, idle CPU/memory, playback/scrubbing, recording and export on the same workload; retain prior measurements as a dated baseline only.
- [ ] Release: Developer ID, hardened runtime, notarization and distribution remain separate from ad-hoc local signing; Windows/Linux builds and device parity require their own evidence.

For each completed gate record revision plus local changes, OS/hardware, command or manual steps, actual result, and artifact paths. Keep historical reports intact; publish fresh GPUI evidence separately. Legacy Slint smoke/architecture scripts outside this migration slice must be ported before their results can be used for GPUI acceptance.

## SubTake design system on GPUI — 21 September 2026

> Superseded by the "Stage" redesign of 22 September 2026. Every number in this
> section — the 40px control height, the 16px radius, the `#5b8cff` blue — was
> replaced by the handoff extracted into
> [DESIGN-PRIMITIVES.md](DESIGN-PRIMITIVES.md), which is the current reference.
> [UI-DESIGN.md](UI-DESIGN.md) describes the system as it stands. This section
> is kept as the record of how the Slint components reached GPUI, not as a
> description of what they look like now. `timeline_scrubber` in the table below
> no longer exists: the redesign's playhead is a rule and a dot, so the widget
> was deleted rather than restyled.

The presentation layer renders SubTake's own design tokens, recovered from the
pre-GPUI Slint sources in git (`ui/theme.slint`, `ui/components/*.slint`)
rather than re-derived by eye. An earlier pass had ported the Zeron
reference's visual language; that is superseded — Zeron's contribution that
remains is the renderer (see below) and the frosted-float element.

### What came across

| Original Slint source | GPUI destination |
| --- | --- |
| `ui/theme.slint` — colour tokens with alpha baked in, metrics | `crates/theme/src/lib.rs` |
| `ui/components/button.slint` — `ButtonSurface` / `ToolButton` / `IconButton` | `Button` in `crates/ui/src/lib.rs` |
| `ui/components/rail-button.slint` | `rail_button` |
| `ui/components/segmented-control.slint` | `segmented_control` |
| `ui/components/scrub-field.slint` + `slider.slint` (filled) | `Slider` |
| `ui/components/timeline-scrubber.slint` | `timeline_scrubber` |
| `ui/components/toggle.slint` | `toggle` |
| `ui/components/panel.slint` — the four surface variants | `panel_variant` / `Surface` |
| `ui/components/input.slint` | `crates/ui/src/input.rs` |

The system: one 40px control height everywhere, a 16px control radius (20px
panels, 24px overlays), 16px glyphs inset 12px, 10/12/14px type, 4/8/12px
gaps, and the `#5b8cff` product blue. Surfaces carry their alpha in the token
(`#fafaf966`, `#ffffff48`, `#e7e7e766` and their dark twins) because they are
tints over the window's vibrancy material, not paints.

The distinctive control is the **scrub field**: a numeric parameter is one
40px plate carrying glyph, caption, level-as-fill and value together, rather
than a label row with a slider beneath it.

### Renderer: the zeronsh/zui fork

SubTake renders on `zeronsh/zui` (rev `c2d273dc`) rather than crates.io
`gpui 0.2.2`. Same crate version and lineage; the fork adds what the frosted
design needs:

* `f596cde` — destination alpha on transparent windows (Porter-Duff OVER, not
  additive). Without it every translucent token renders see-through instead
  of tinted, which would collapse the whole palette.
* `8a8954c` — macOS blurred view on `UnderWindowBackground`; macOS 26 stopped
  vending `CABackdropLayer` for the `Selection` material stock gpui requests.
* `BackdropBlur` → `Window::paint_backdrop_blur`, used by `crates/ui/src/frost.rs`
  (adapted from the Zeron reference, MIT © 2026 Wing) for menus and tooltips.
* `EdgeFade` → `Window::with_edge_fade` (available, not yet applied).

The fork splits the platform layer out, so the manifest also carries
`gpui_platform` (which owns `font-kit` and `runtime_shaders`). API drift
handled during the switch, all mechanical:

| Stock 0.2.2 | Fork |
| --- | --- |
| `Application::new()` | `gpui_platform::application()` |
| `window.focus(handle)` | `window.focus(handle, cx)` |
| `focus_next()` / `focus_prev()` | now take `cx` |
| `AsyncApp::update` returns `Result` | returns `()` |
| `Line::paint(origin, height, window, cx)` | adds `TextAlign` + `Option<Pixels>` |
| `Menu { name, items }` | adds `disabled` |

The editor window is `WindowBackgroundAppearance::Blurred`; gpui installs the
blurred view itself, so SubTake's own `subtake_window_set_blur` helper is no
longer applied to the editor. The recorder keeps its native *masked* glass,
which gpui cannot express.

### Deviations

* **Interface font** follows the original token (`Helvetica Neue`). The Geist
  faces stay bundled and registered so a theme can opt into them.
* **Timeline position** keeps a scrub field where the design reference shows a
  plain scrollbar; it is a real control in SubTake.
* **Scroll-edge fades** are applied to the rail, the inspector body and the
  timeline tracks via `subtake_ui::fade_edges`. They use the fork's
  `Window::with_edge_fade`, which multiplies each primitive's alpha by a
  vertical ramp, because the usual trick — an overlaid gradient in the
  backdrop colour — has no colour to use over a blurred window. Each region
  also carries `FADE_BAND` of vertical padding so the ramp falls on empty
  space when nothing is clipped.

### Evidence

`test-output/zeron-visual-review/` — editor, minimum size, inspector panels
and recorder states in both appearances, plus glass evidence. 77 workspace
tests and all 11 packaged UI checks (including visible-label OCR) pass. Real
mouse and keyboard were exercised against the packaged build: rail click
switches panel, slider drag commits and marks the document dirty, `Cmd+Z`
undoes and enables Redo, and a timeline click seeks with the scrubber and
preview following.

## Achromatic control language — 21 September 2026

The interface no longer has an accent colour. Every control is white, grey or
black at some opacity, either **filled** or **washed**, and all the colour a
user sees comes from the desktop through the frost. The `blue` token is gone;
`Theme::accent` is now the filled emphasis plate and a unit test asserts every
control colour has zero saturation.

| token | light | dark |
| --- | --- | --- |
| `accent` (filled plate) | `#242424` | `#f0f0f0` |
| `accent_hover` | `#3b3b3b` | `#ffffff` |
| `on_accent` (glyph on the plate) | `#ffffff` | `#171717` |
| `selection` (wash) | black @ 10% | white @ 12% |
| `border` (outline) | black @ 11% | white @ 13% |

A control is filled when it is a primary action or switched on, and washed
otherwise — one rule covering buttons, rail items, segmented controls,
recording-overlay pills and the toggle. The toggle's thumb takes `on_accent` when the
track is filled, or it would be white-on-white in the dark appearance. Focus
rings likewise flip to `on_accent` on a filled plate.

Timeline track tints stay coloured: they identify a clip's category and are
content, not chrome. An achromatic interface is what lets them read.

## Closed type and icon scales

Sizes are tokens only — five type steps (`FONT_SMALL` 10, `FONT_CONTROL` /
`FONT_BODY` 12, `FONT_HEADING` 14, `FONT_DISPLAY` 22) and three icon steps
(`ICON_SIZE_SMALL` 12, `ICON_SIZE` 16, `ICON_SIZE_LARGE` 20). The root element
sets `FONT_BODY`, so most elements set no size at all. Previously the root
default was 11.5 with 10, 11, 13 and 22 sprinkled through the views, and icons
ran 12/18/26/28.

`scripts/check-ui-primitives.py` now fails the build on a numeric literal
passed to `text_size`, `glyph_size` or `icon_sized` outside the theme crate,
so the scales stay closed. `examples/` is exempt — the font probe renders
off-scale specimens on purpose.

## Shared primitives

Marks that had been hand-rolled in `gpui_views.rs` moved into
`crates/ui`: `status_dot` (the unsaved mark), `progress_bar`, `swatch`,
`media_tile` and `empty_state`, with their metrics (`DOT_SIZE`,
`PROGRESS_HEIGHT`, `SWATCH_SIZE`, `TILE_WIDTH`, `TILE_HEIGHT`) as theme
tokens. The views now compose primitives rather than styling divs.

## Motion — 22 September 2026

The reference carries a full motion catalog (`crates/ui/src/motion.rs`, 1164
lines): entrance fades, menu and dialog entrances, resize transitions, a
splash exit, loader pulses, and Tailwind `transition-colors` parity on every
interactive wash. The port took only the last of those, and until now it was
dead code — `hover_blend` and `hover_listener` had no call sites, so every
control snapped between its states through gpui's immediate `.hover()` style.
That is what made the interface feel static.

What is wired now:

| Surface | Motion |
| --- | --- |
| Button, dropdown trigger, dropdown row, choice tile | Hover wash fades over 150 ms on `cubic-bezier(0.4, 0, 0.2, 1)` |
| Button fill, choice tile, swatch, rail caption | Selected state cross-fades over the same curve |
| Toggle | Track colour, thumb colour and thumb travel on one 150 ms progress |
| Every pressable control | `PRESSED_OPACITY` while held — immediate, not faded |
| Dropdown menu, command menu | 140 ms `ease_out_quint` fade with a 3 px settle |
| Tooltip | 140 ms fade |
| Slider, timeline scrubber | None, deliberately: a drag must stay 1:1 with the pointer |

Two tween drivers share one store. Hover is an *event*, so it hangs off an
`on_hover` listener. Selection, switch state and the rail's active panel are
states the control *holds* with no enter/leave event to hang on, so
`state_fade` drives them from render — which is why it re-anchors only when
the target changes, and adopts its value outright the first time a key is
seen (a panel that opens with a switch already on must not play the
switch-on animation).

Constraints worth recording:

- gpui at the pinned revision has no scale transform for divs, only for
  `svg`. The reference's `scale(0.96) → 1` menu entrance is approximated with
  fade plus a 3 px vertical settle, applied to `top` — which means `menu_in`
  is only safe on an absolutely positioned surface.
- `tick_hover_fades()` runs at the *end* of `RootView::render`, not the
  start. Hover fades get their frames from the `window.refresh()` in the
  listener, but a tween a control starts from its own render is invisible to
  the store until that render has happened, and nothing else would request
  the frames needed to finish it.
- Reduce-motion is honoured on both paths: the tween store snaps when
  `reduced_motion()` is set, and gpui's `AnimationExt` skips scheduling
  frames for the entrances on its own.

Still absent relative to the reference: the resize/collapse transitions for
panels and the rail, tab slides, and the shared `PulseClock` that the
reference introduced to keep repeating loaders off a 120 Hz repaint loop. We
have no repeating loaders yet, so the clock is not needed until we do.

## Overlays — composer layout, 22 September 2026

The three transient surfaces — the command menu, the recording overlay
and the recorder options window — now share one structure, taken from the
Claude Code composer the reference screenshots show: a grey context chip
naming the surface, the working controls beneath it, and a quiet icon row as
a footer. Two primitives carry it, both in `crates/ui/src/lib.rs`:

- `context_chip(theme, parts)` — `CHIP_HEIGHT` (24 px), `RADIUS_SMALL`,
  surface fill, `FONT_SMALL` muted text, parts joined with a separator.
- `composer_footer(theme)` — a `FOOTER_HEIGHT` (28 px) row at `FONT_SMALL`
  and muted, for icon controls and a hint.

No primitive changed colour: both reuse existing `surface` and `muted`
tokens.

### Command menu

`RootView::menu_overlay` was a fixed-position list. It is now a palette:

- It anchors to its trigger. `menu_button` wraps the control in a `measure`
  that writes the trigger's bounds into `RootView::menu_anchor`, and the card
  opens under those bounds, clamped to the viewport and flipped above the
  trigger when the card would not fit below. The old fixed coordinate put
  the card at the top of the window while the trigger sat in the timeline
  strip at the bottom.
- It filters. The search field is a `TextInput` with a live `set_on_change`
  that writes `RootView::menu_filter`; enter runs the first match. The field
  takes the keyboard when the card opens (`menu_focus`), so filtering starts
  with a keystroke rather than a click.
- Its footer switches menus, which is the first entry point File, Edit and
  Help have had inside the window.

Rows carry no glyph: a third of the commands have no icon in the bundled
Phosphor set, and inventing one per row reads worse than a clean list.

Escape needed a new hook. `TextInput::cancel` stops propagation, so a focused
field inside a transient surface swallows the key that was meant to close
it; `set_on_cancel` hands it back, and the palette closes on it.

### One command table

`gpui_views::menu_commands` is now module-level and shared: the palette reads
it, and `ui_runtime::install_menus` builds File/Edit/Help from the same
groups, inserting a separator between them. A `@`-prefixed command opens a
panel on both paths instead of dispatching an action.

Only ⌘Q/⌘O/⌘S/⌘⇧S/⌘Z/⌘⇧Z are bound in the gpui keymap. Cocoa consumes an
NSMenuItem key equivalent in `performKeyEquivalent:` *before* the window sees
the key down, and gpui derives the menu's equivalents from the keymap — so
binding ⌘C/⌘X/⌘V/⌘A there would steal them from a focused `TextInput`. Those
keys keep reaching the editor through `invoke_keyboard`, as they already did.

### Recorder surfaces

The recording overlay's bare `⠿`, `?` and `×` characters had no hit target between
them; they are now icon controls at `CONTROL_HEIGHT`
(`DotsSixVertical`, `Question` with the status tooltip, `X`). The options
window's title row became a context chip plus a ghost close icon, and the
sources panel's muted hint moved into a `composer_footer`. Both moved off
raw tailwind spacing onto `GAP`/`GAP_LARGE`.

The options window sizes are unchanged: the tallest panel ("more") lays out
to about 232 px against a 284 px window, and "sources" to about 228 px
against 264 px, so the chip and footer both fit inside the existing defaults
in `ui_state`.
