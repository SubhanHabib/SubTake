# Native editor visual design

The September 13 refresh follows the two light video-editor reference screenshots supplied by the user: neutral surfaces, rounded controls, a dotted preview canvas, a right-hand inspector, filled numeric sliders, soft timeline colors and a larger labelled filmstrip. This is a Slint implementation; no web view was added. Existing SubTake actions and project keys remain connected to the native editing core.

The header is 56px tall with centered project title, project/save/history controls, recording, presets and a lime Export action. The left rail uses circular icon controls with captions. The inspector is 280px wide and scrolls independently. The timeline has a 240px minimum and 320px preferred/maximum height so its bottom controls stay inside a 980x680 window. Effect lanes scroll, and trim handles retain full-height hit targets around their smaller visible grips. The rounded source strip and neutral playhead share the same time mapping as the tracks.

`ui/theme.slint` owns light/dark colors and sizing. `ui/components/scrub-field.slint` combines the filled slider with an editable value; dragging commits on release and typed values commit on Enter. Buttons, dropdowns, toggles, checkboxes and other primitives remain in `ui/components/` and are exported through `ui/controls.slint`. Phosphor icon geometry is unchanged. The full component map and gallery workflow are in `UI-COMPONENTS.md`.

The reference's AI brightness/upscale labels do not introduce new product features. Editor and recorder surfaces are translucent over native Mac window blur. The six-dot timeline handle uses blurred filmstrip artwork plus a translucent dark tint; it has no opaque gradient. The design adapts the supplied reference to SubTake's recording, cursor, caption and effect workflows; it does not claim a pixel-identical clone.

Visual review uses native Slint window snapshots at 1360x880 and 980x680, light/dark component galleries, and recorder overlay snapshots. Pointer/key checks exercise the rail, shared controls and editing workflows. GUI tests must run sequentially: competing windows can steal focus from a dropdown popup. `docs/ui-validation.json`, `docs/launcher-validation.json` and `docs/primitives-validation.json` hold the latest machine-readable results. An isolated landscape project in `test-output/design-review/` uses a bundled wallpaper for a less distracting live comparison. User projects are not changed by that demo.

## Recorder entry point

The entry point now follows `legacy-electron/src/components/launch/LaunchWindow.tsx` and its CSS in the reference implementation: a compact rounded floating bar, a source control, microphone/system-audio options, webcam options, countdown, a red Record control, More and Hide. Native option panels expand above the bar. The same overlay displays countdown, recording elapsed time, pause/resume and stop/finalization.

macOS starts in accessory mode with an icon-only menu-bar item and Open/Quit. The editor is hidden at startup. Recording saves automatically to Movies/SubTake or a chosen folder; finalized media opens the editor and hides the recorder. Closing/hiding a window does not end the process. The overlay's Slint UI, settings callbacks and lifecycle are shared; macOS activation policy, screen positioning and capture protection live in the platform bridge.

The initial macOS window handle is only requested after the event loop starts. Source discovery is independent of that positioning step. This avoids an overlay that appears but never enumerates capture sources.

## Timeline scrubbing

The ruler and filmstrip seek on pointer down, continue scrubbing while held, and commit the release position, clamped to the visible time range. The playhead is a single overlay with a 26px hit target above the effect regions, so dragging it does not select an underlying effect. Its reusable `TimelineScrubber` component follows the supplied narrow reference: a pointed grey cap, continuous 2px rule, shaded rounded grip and six white dots. It stays continuous across the filmstrip and visible lanes when the track content scrolls. `cargo run --offline --locked --example timeline_interaction` exercises those pointer paths and the right boundary.

## Timeline pinch zoom

Pinch over the timeline to zoom between 1x and 100x. The time under the pointer at gesture start stays anchored while the visible range changes; the range clamps at either end of the source. Pinching outside the timeline does not change it. Slint 1.17.1 routes native macOS trackpad pinch events through `ScaleRotateGestureHandler`; the shared handler also follows Slint platform gesture support. Windows/Linux trackpad parity has not been tested. Gesture cancellation keeps the last applied view and allows the next gesture to begin normally. Pinching only changes the viewport, not project content or the playhead time.

The native timeline regression example injects the same core pinch events emitted by the winit backend. Its test-only dependency on `i-slint-core` is pinned to the installed Slint version. These automated checks do not substitute for a physical trackpad test.

## Preview magnification

Pinching over the preview magnifies the workspace view from Fit (1x) to 8x. The image position under the gesture remains anchored, subject to image-edge bounds. Two-finger scrolling pans the magnified view, and the Fit control resets scale and position. `PreviewViewport` owns this behavior; image and edit-overlay children share the same scaled coordinate system. Opening a project or changing preview aspect resets the view. These transient UI properties are separate from project effects, crop settings, playhead time and exported framing.

Native gesture regression checks cover preview anchoring and normalized click coordinates, scroll panning, bounds, Fit and isolation from the timeline. Physical trackpad input remains a manual acceptance check.

## Uniform control height

Buttons, dropdowns and sliders share the filled slider's 40px height in the editor and recorder. Header, inspector and toolbar rows accommodate that height; the rail scrolls in short windows. Recorder option panels are 264px tall inside a 386px expanded window, while the collapsed bar remains 106px tall. The timeline filmstrip can shrink on small windows so its controls remain visible.
