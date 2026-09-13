# Native parity and release status

Date: 12 September 2026. Reference: parent Electron checkout at `7512ef1518657edc2caae595136821d27a47167d`.

**Implemented means a working code path exists. Verified means the stated check was run. Neither word alone means every edge case or pixel matches Electron.** This is a substantial runnable native Mac implementation, not a declaration that every current product behavior has been ported and certified.

## Feature implementation and evidence

| Area | Implementation | Evidence / limits |
|---|---|---|
| Native UI | Recordly-style Slint header, left rail/inspector, transport, menus, timeline and a separate menu-bar recording overlay; Phosphor icons, typed selectors and wallpaper grid | Packaged pointer-event checks, eleven editor snapshots, six recorder layouts and a complete screen-only recording handoff; accessibility/keyboard/screen-reader acceptance still required |
| Documents | Version 1/2, unknown fields, file URLs, relative assets, atomic saves, previous-file backup, undo/redo, missing source relink, recent files, chosen-folder library/search | Automated roundtrip, invalid transaction, backup, path and history checks |
| Recovery | Background snapshots after edits, recover from Recent, save/discard cleanup, retained failed recording directories | Snapshot and related-file replacement tests; kill/crash recovery UX still needs an end-to-end drill |
| Timeline | Trim union, clips, speed, split, delete, both-edge resize, group selection/movement, copy/cut/paste/duplicate with clip audio settings, markers, zoom/pan, ruler, snapping, thumbnails and waveform | Mapping/editing tests and native UI smoke; full interaction parity with the existing timeline is not established |
| Camera/zoom | Manual/auto focus, connected holds, easing, damped springs and cursor follow | 106 production zoom-strength cases; deterministic seek test; canonical 60 Hz sampling differs from the reference's stateful frame stepping |
| Auto zoom | Explicit-click clusters, trajectory strengths, overlap avoidance, fresh landscape recording suggestions | Ten generated production-reference cases; portrait restriction on Mac preserved |
| Frame/background | Visual crop editor, aspect, linked/independent padding, squircle corners, shadow, image/video background, background blur, hex linear gradients, runtime wallpaper discovery | 24 geometry reference cases; crop bounds and UI undo checks; generated scene/media smoke; exact shadows and arbitrary CSS gradient fidelity remain unverified |
| Cursor | Tahoe/macOS/Windows 11/dot/Figma assets, size, smoothing, spring sway, loop, click bounce, ripple/spotlight/echo, shadow and directional blur | Spring-reference cases, five asset-family renders, deterministic seeks; full pixel/motion differential remains outstanding |
| Camera blur | Native Skia runtime shaders porting directional/radial blur and camera-step classification | Shader compilation and composed MP4/GIF smoke; pixel equivalence and complete tuning-reference corpus still outstanding |
| Webcam | Native companion capture, imported/replaced footage, crop, mirror, size, presets/custom placement, margin, roundness/shadow, zoom reaction and time offset | Generated synchronized webcam fixture; actual devices, pause/resume and drift are not yet validated |
| Annotations | Text, image, arrow and blur, preview drag/resize, positions/sizes, fonts, colors, alignment, weight/style/underline, z-order | Render smoke; direct manipulation is implemented but the complete existing selection/cycling/crop workflow still needs acceptance |
| Captions | SRT import, local Whisper/model selection/download/language, token reconstruction, paged layout, animation, font import, sentence/silence segmentation, split/merge, word text/timing edits, burn-in, SRT/VTT output | Official Small model downloaded and recognized generated speech from a mic-only sidecar; five parser reference cases, 270 phrase segmentation cases, 24 split/merge comparisons and 288 layout/animation cases; multilingual/long-recording acceptance still required |
| Audio | Embedded/companion source tracks, per-track/per-clip settings, additional clips, volume, normalization behavior, trims/speed, limiter, preview audio clock | Spectral mix checks verify system/mic/music levels and prevent duplicated embedded/system audio; the real default output-device clock and cancellation passed using silence; audible/perceptual synchronization and device-change acceptance still needed |
| Exports | Shared compositor, MP4/H.264, VideoToolbox option, GIF palette, size/fps/quality/loop controls, progress/cancel, atomic final video, reveal | Software and VideoToolbox paths each produce 90 video frames / 3 seconds with 3-second audio; 30 GIF frames; existing destination survives cancellation; 192 production export-dimension cases |
| Mac capture | ScreenCaptureKit display/window recording, system audio, AVFoundation mic/camera device selection, countdown/cancel, pause/resume/stop, telemetry, error recovery | Packaged overlay startup, hide/Open, countdown cancel, recording, pause/resume/stop and automatic editor handoff passed using generated window content. **Real mic/camera/system-audio integration and long-session synchronization remain unverified** |
| OS workflow | File dialogs/drop, Finder document event bridge, native tray, configurable global/editor shortcuts, fullscreen HUD policy, help/feedback | Code builds; Finder opening, fullscreen/Spaces, sleep/wake and shortcut conflicts need live Mac acceptance |
| Packaging | Optimized Apple Silicon .app with assets, FFmpeg/FFprobe, dylibs, Whisper and capture helpers | Ad-hoc signed and deep/strict signature verified; not Developer ID/notarized, not Intel/Universal validated |

## Deliberately inactive in the current reference

- The extension marketplace/runtime is disabled in the active product revision; a full extension host is not included in this native build.
- Per-word caption highlighting is disabled in the active caption renderer; word timing still controls layout/pages.
- `TEMPORAL_ZOOM_MOTION_BLUR_ENABLED` is `false` in both production exporters. The native renderer preserves the settings but implements the active spatial blur path.

## Unfinished shared product work — affects Mac and Windows

These items must not be hidden in the Windows-only backlog:

1. Complete visual differential testing for combined zoom/cursor motion, shadows, blur, fonts, caption geometry and webcam transforms. The native canonical motion clock deliberately makes seeks deterministic; exact reference trajectories at every export FPS have not been proven.
2. Complete manual interaction acceptance of multi-selection, annotation cycling, preview manipulation, visual cropping, folder browsing and word editing. Visual crop with undo, searchable folder indexing, and preset save/apply/removal are implemented in the shared code. Removed presets retain a recoverable local copy. The existing reference preset workflow does not require a separate rename action.
3. Complete translation coverage for new native labels and dialogs. The existing ten product catalogs are embedded, with a persistent language setting; shared labels translate and unmatched native labels fall back to English. Transcription language is independent.
4. Test very long speech, multilingual token timing and cancellation during a long inference job. Silence-based postprocessing, sentence boundaries, abbreviation handling and rapid-caption merging are implemented and match the generated reference corpus.
5. Complete VFR/rotation/HDR/long-GOP/long-session/media-loss validation and sustained playback synchronization. The in-process FFmpeg decoder seeks by stream timestamps, but the shared request clock still quantizes to the probed frame rate; it uses software decoding with RGBA upload/readback, not a zero-copy VideoToolbox/D3D pipeline.
6. Native update/distribution lifecycle, release feed, signing/notarization/installer policy and complete third-party notices are not implemented as a production release system.
7. Validate full accessibility, app close/reopen/recovery, failure reporting, and every configurable control on the packaged application.

## Mac acceptance still required

- Grant Screen & System Audio Recording access to SubTake, and microphone/camera permission when enabled.
- Record display and window at Retina/mixed scaling, move/resize the target window, verify cursor coordinates and that the editor/HUD are excluded.
- Record mic only, system only, both, camera only, and camera plus both audio sources. Verify A/V alignment at start, after pause/resume, and after a 10-minute take.
- Verify global shortcuts, menu-bar controls, fullscreen Spaces, sleep/wake, device removal and permission revocation.
- Verify native playback audio/device changes, Finder opening, file drops, dirty-save cancellation and a forced-crash recovery drill.
- Run on macOS 14 and current stable macOS, and build/test Intel if it remains a supported Mac target. Current host is macOS 27.0 build 26A5421a, Apple M5 Pro, 48 GiB RAM.
- Complete a matched release performance comparison before claiming this implementation is faster or uses less battery than Electron.

Automated checks and measurements are retained in `VALIDATION.md` / `validation.json`; generated media and detailed logs live in ignored `test-output/`.
