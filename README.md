# SubTake

A native Rust/Slint desktop editor with a shared Skia compositor and an in-process FFmpeg decoder. macOS recording uses ScreenCaptureKit and AVFoundation helpers. There is no Electron, React, Tauri or WebView in this application.

**Status: runnable local Mac implementation; full production parity is not yet certified.** The implementation and automated evidence are described in [PARITY.md](docs/PARITY.md). The screen-only recording flow has passed; real microphone/camera/system-audio combinations and long-session validation remain outstanding. [WINDOWS-HANDOFF.md](docs/WINDOWS-HANDOFF.md) lists the Windows work, and explicitly separates unfinished shared work from Windows-only work.

The Rust/Slint application is the main project at this repository root. The previous Electron/React app, its dependencies, original automation and architecture spike are preserved in [`legacy-electron/`](legacy-electron/README.md). Its workflows are archived under that directory and are not active root GitHub Actions. Shared reference assets and the cursor/Whisper build inputs still come from `legacy-electron/`; the packaged native app is self-contained.

- Root: `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/`, `ui/`, `assets/`, `scripts/`, `tests/`, `docs/`.
- Native package: `dist/SubTake.app`.
- Previous app: `legacy-electron/`, including its `node_modules/` and `spike/`.

## Run the Mac app

The locally built application is `dist/SubTake.app`. It targets macOS 14 or later; the build verified on this machine is Apple Silicon.

```sh
open -n dist/SubTake.app
open -n dist/SubTake.app --args /absolute/path/to/project.recordly
```

Normal launch opens the floating recorder and menu-bar icon. Choose a source, press Record and then Stop to open the captured project in the editor. See [RECORDING.md](docs/RECORDING.md). You can also open a video or `.recordly` / `.openscreen` project. The left rail opens Scene, Cursor, Webcam, Captions, Audio and Settings. The header opens Projects, Presets and Export. Scene includes a visual Background picker; selecting a timeline region opens its inspector. See [UI-DESIGN.md](docs/UI-DESIGN.md) for the Recordly layout references and native controls. Fields accept Enter to commit; sliders commit on release. Timeline zoom/position controls, snapping, source thumbnails and the waveform support longer recordings. Shift-click regions to select a group; Cmd+A selects all regions. Drag a region to move the selection; drag its left/right edge to resize that region. Copy/cut/paste/duplicate preserve relative timing and per-clip audio settings. Select an annotation, or open Webcam, to drag its preview outline or resize using its bottom-right handle. Crop above the preview shows the whole source with a draggable crop rectangle; Done returns to the composed preview. Recent can index and search a chosen folder of projects and recordings. The playhead and region coordinates use the source clock; export removes trims and applies speed changes.

Record selects a display or window and optionally a microphone, system audio and a camera. macOS must grant Screen & System Audio Recording access; microphone/camera permissions are needed when those devices are enabled. The app requests them through its native helpers. It cannot grant its own permissions. Global record/stop and pause/resume bindings are configurable in Preferences, alongside six editor actions. Help contains the shortcut reference.

Captions run locally using the bundled `whisper-cli`. Download Whisper Small from Captions or select an existing GGML model. The model is stored in Application Support, separately from the app and projects. No recording is uploaded for transcription. Silence and sentence analysis produce readable phrases. Select a caption to split near the playhead, merge with the following caption, or edit individual word text and times. SRT import and optional SRT/VTT export are supported.

Appearance presets can be saved, loaded and removed from Presets in the header; removed files are retained under `presets/.trash/`. Saved preset files in the application-support `presets/` directory appear in the Presets panel. Applying a preset retains the source recording, webcam source and timeline edits. Focused and Smooth motion presets are available in Cursor. The Edit menu cycles between annotations at the playhead. Settings selects one of the ten existing interface languages; labels reused from the product catalogs are translated, while new native labels currently fall back to English.

## Build

Requirements: Rust/Cargo, Xcode Command Line Tools, Python 3, FFmpeg 7 or newer development headers/libraries, FFmpeg and FFprobe. The default `native-ffmpeg` feature links libavformat, libavcodec, libavutil and libswscale. Set `FFMPEG_DIR` to a prefix containing `include/` and `lib/` if they are not in the usual Homebrew prefix. `--no-default-features` retains the slower subprocess decoder for diagnosis. The Mac package script also uses the existing cursor and Whisper binaries in `legacy-electron/electron/native/bin/darwin-arm64` (or `darwin-x64` on Intel).

```sh
cargo build --locked
cargo run -- /absolute/path/to/video.mp4
python3 scripts/build-mac.py --release
```

The package script compiles the Mac capture helpers, copies assets and runtime binaries, relocates their Homebrew dylib dependency closure, signs the bundle and verifies its signature. It stages the entire package before replacing the previous local bundle. Local signing defaults to an ad-hoc identity. `--identity` selects a signing identity, but **Developer ID distribution, hardened-runtime/notarization and an updater are separate unfinished release work**; the local build is not a public notarized release.

Runtime overrides: `SUBTAKE_RESOURCES`, `SUBTAKE_FFMPEG`, `SUBTAKE_FFPROBE`, `SUBTAKE_WHISPER_CLI`. `SUBTAKE_RENDERER=software` forces the shared CPU compositor for diagnosis. A packaged app finds its bundled runtime before searching PATH.

## Shared implementation

| Module | Responsibility |
|---|---|
| `project.rs`, `editing.rs`, `recovery.rs`, `file_group.rs` | Version 1/2 documents, unknown-field preservation, atomic saves/backups, history, portable asset resolution, timeline edits, recovery snapshots, related-file replacement |
| `timeline.rs`, `geometry.rs`, `motion.rs`, `effects.rs`, `autozoom.rs` | Source/output clocks, crop/frame layout, deterministic camera/cursor springs, spatial blur, explicit-click zoom clustering |
| `media.rs`, `native_decoder.rs`, `scripts/decoder.c`, `render.rs`, `export.rs` | Runtime discovery, probing/decoding, native composition, audio mixing/playback, MP4/GIF export, cancellation |
| `captions.rs`, `caption_editing.rs`, `segmentation.rs`, `transcription.rs`, `models.rs`, `subtitles.rs` | Caption layout/animation, local Whisper, model download, subtitle import/export timing |
| `preferences.rs`, `library.rs`, `presets.rs`, `localization.rs`, `shortcuts.rs`, `app.rs`, `ui/editor.slint`, `ui/controls.slint`, `inspector.rs` | Preferences, bindings, native editor, timeline, inspector, tray and recording controls |
| `platform.rs`, `scripts/*.swift`, `scripts/file-events.m` | Platform boundary: capture, telemetry/devices, fullscreen HUD and Finder document events |

The shared Rust and Slint code is used directly by the Mac build and is the implementation Windows should retain. Windows capture methods currently return explicit unsupported errors. No Windows or Linux build has been executed here.

## Verification

```sh
cargo test --locked
# Optional device check: opens the real audio output, playing silence only.
cargo test --locked --test media_integration silent_output_device_clock_and_cancel -- --ignored
node scripts/reference-fixtures.mjs  # requires the legacy-electron/node_modules
python3 scripts/localization.py  # refresh embedded translations from the legacy-electron catalogs
python3 scripts/smoke.py
SUBTAKE_TEST_BINARY="$PWD/dist/SubTake.app/Contents/MacOS/SubTake" python3 scripts/smoke.py
SUBTAKE_UI_SNAPSHOT="$PWD/test-output/editor.png" dist/SubTake.app/Contents/MacOS/SubTake test-output/fixture.recordly
```

`reference-fixtures.mjs` imports the actual production TypeScript to generate the reference corpus. The media smoke uses generated test patterns and sine tones, and never captures the desktop. The UI smoke exercises document loading, region editing, undo/redo, clipboard commands, clip splitting, seeking and a native window snapshot. `SUBTAKE_UI_SIZE=980x680` exercises the minimum window size.

The CLI also supports `probe`, `validate`, `render`, `export`, `sources`, `download-model`, `transcribe` and `benchmark`; use `--help`. Benchmark output measures decoding, composition and pixel readback, and excludes UI presentation and audio. It is not an Electron comparison.

## Files and recovery

Projects preserve fields the native editor does not understand. Native-only export settings use `nativeExport*` keys so Electron's `exportQuality` and `exportEncodingMode` remain intact. Recognized relative media paths resolve against the document directory. Bundled `wallpapers/...` references stay portable.

User settings and the downloaded model live under the platform application-support/configuration directory. On Mac: `~/Library/Application Support/com.SubTake.SubTake-Native/`. Debounced recovery snapshots are in its `recovery/` folder and can be opened from Recent. Recovery does not replace the saved document. Saving or explicitly discarding a document clears its snapshot.

A recording produces the video, optional `.system.m4a`, `.mic.m4a`, `.webcam.mp4`, and `<video>.cursor.json` companions. A failed finalization retains its `SubTake-recording-*` work directory and reports the path. `session.json` identifies the requested recording and `commit.json` plus `previous-recording/` identify replacement/rollback files. On a companion failure the screen recorder is still asked to finalize. Successful replacement removes stale companion tracks from the older take.

## License

AGPL-3.0-only, preserving the Recordly/OpenScreen lineage and the original motion work by @webadderall. The native package includes the repository license and attribution. Third-party components retain their own licenses; distribution needs the complete dependency/runtime notice and source-compliance review described in the release checklist.

## UI component system

The editor and recorder share the primitives exported from `ui/controls.slint`, with implementations in `ui/components/` and tokens in `ui/theme.slint`. See [UI-COMPONENTS.md](docs/UI-COMPONENTS.md) for the component map, variants, interactive gallery, architecture checks and transparency/blur boundaries.
