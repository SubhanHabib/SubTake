# Windows completion ledger

> GPUI migration notice: UI descriptions and validation evidence below predate the migration and are retained as historical reference. They do not certify current GPUI behavior. See [GPUI-MIGRATION.md](GPUI-MIGRATION.md) for current example coverage and pending acceptance gates.

Date: 12 September 2026. Native implementation: repository root. Legacy reference: `legacy-electron/`.

**Windows recording is not implemented or tested.** The shared editor/media code is present and exercised on Mac; no Windows build, device test or installer has been run. Keep the shared implementation; replace the OS boundary and validate it on Windows. The unfinished shared work in [PARITY.md](PARITY.md) still applies to both platforms.

## Shared code to retain

| Shared area | Files | Windows disposition |
|---|---|---|
| Project model/history, asset paths, saves/backups and recovery | `src/project.rs`, `editing.rs`, `recovery.rs`, `file_group.rs` | Implemented, including preservation of Electron fields and stale companion cleanup; run Windows filesystem/path/lock tests |
| Timeline math, geometry, motion, auto zoom | `src/timeline.rs`, `geometry.rs`, `motion.rs`, `autozoom.rs` | Same implementation and reference corpus; no parallel Windows rewrite |
| Composition and effects | `src/render.rs`, `effects.rs`, `captions.rs` | Shared drawing/effect code; Windows currently uses the CPU composition fallback, requiring GPU integration/performance work below |
| Decode, audio, export and subtitles | `src/media.rs`, `native_decoder.rs`, `scripts/decoder.c`, `export.rs`, `subtitles.rs` | Shared FFmpeg/audio/MP4/GIF/cancellation logic; platform runtime/codec availability must be supplied and tested |
| Local transcription and model lifecycle | `src/transcription.rs`, `segmentation.rs`, `caption_editing.rs`, `models.rs` | Shared parsing, audio-source selection, HTTPS download, atomic storage, cancellation; bundle a Windows Whisper runtime |
| UI, settings, shortcuts and project workflow | `ui/editor.slint`, `src/app.rs`, `preferences.rs`, `library.rs`, `presets.rs`, `localization.rs`, `shortcuts.rs` | Shared Slint UI and controller; tray/global-hotkey code already exists and needs Windows integration testing, not a duplicate implementation |

## Windows-specific implementation still required

| ID | Work | Entry points / reusable reference | Completion check |
|---|---|---|---|
| W01 | Implement capture source enumeration | `platform::sources()` currently returns an explicit error. Use monitor/window enumeration from `legacy-electron/electron/native/windows-capture/src/monitor_utils.*` and the existing Electron Windows platform layer | Stable monitor/HWND identities; exclude this app; mixed DPI, negative monitor coordinates, minimized/closed targets and hot plug |
| W02 | Implement the Windows recording session | `Recording::start()` currently rejects Windows. Bind/build the existing WGC/DXGI helper in `legacy-electron/electron/native/windows-capture/` | First-frame readiness, typed failure output, bounded start/cancel, pause/resume acknowledgements, stop/finalize and orphan cleanup match the Mac contract |
| W03 | Implement system audio and microphone capture/device selection | `platform::devices()` currently rejects Windows; reuse `wasapi_loopback.*` and add/verify microphone endpoint handling | System-only, mic-only, both, endpoint changes, exclusive devices, unplug, mute and no duplicate embedded/sidecar audio |
| W04 | Implement camera companion capture | Mac `scripts/companion.swift` has no Windows equivalent; use Media Foundation or another native Windows device backend behind the same session interface | Device IDs, start/stop, mirror is a render setting, output dimensions, timestamps, pause compensation and finalization |
| W05 | Implement cursor/input telemetry | Replace Mac `Telemetry` / `subtake-platform telemetry` and cursor classification with Windows native hooks | Same JSON source-clock samples at a suitable rate, normalized selected-source coordinates, click/double/right/middle/up events, shape classification, pause trimming and movement across monitors |
| W06 | Implement Windows HUD window policy | `configure_recording_hud()`, `position_launcher()` and `set_editor_active()` implement the Mac window policy only. Shared `RecordingLauncher` setup/recording overlay, icon menu, persistent event loop and global shortcuts already exist | Topmost behavior, no unwanted activation, DPI placement, taskbar/Alt-Tab behavior, `SetWindowDisplayAffinity(WDA_EXCLUDEFROMCAPTURE)` where supported, fullscreen and shortcut conflicts |
| W07 | Add a GPU compositor backend | `Scene::create_surface()` is Metal on Mac and CPU elsewhere | Select a supported Skia Windows backend; shader support, context loss, Intel/AMD/NVIDIA and software fallback. Validate output before optimizing interop |
| W08 | Add Windows hardware encoding | `export()` explicitly rejects `hardware=true` off Mac | Probe/select Media Foundation/NVENC/QSV/AMF as appropriate, handle unavailable/busy encoders, codec dimensions/fps constraints, quality mapping, cancellation and A/V correctness |
| W09 | Package all native runtimes and assets | Implement a Windows counterpart to `scripts/build-mac.py`; `media::binary()` already resolves `.exe` | Implement the Windows resource-directory layout (the default resource root currently follows a Mac bundle), then build against FFmpeg 7+ development headers/import libraries using `FFMPEG_DIR`; bundle libavformat/libavcodec/libavutil/libswscale DLLs and their dependencies alongside FFmpeg/FFprobe, Whisper, capture/input helpers, dependent DLLs, assets/fonts, app icon and licenses work on a clean machine without Cargo/Node/Python or developer PATH |
| W10 | Integrate Windows application lifecycle | Replace the Mac Finder bridge; retain shared file dialogs/drop/reveal | `.recordly` / `.openscreen` associations, second-instance file forwarding, clean shutdown, tray restart, locked files and Windows path casing/Unicode/long paths |
| W11 | Create signed installer/update integration | No native release installer/updater is shipped | Choose installer and update mechanism, register/unregister associations, versioned upgrades/rollback, code signing, uninstall and user-data retention |
| W12 | Add Windows CI and release validation | `cargo test --locked`, `scripts/smoke.py`, reference fixtures; Windows runtime prerequisites required | Build with Windows SDK/MSVC, execute tests on Windows 10 build 19041+ and Windows 11, verify actual GPUs and recording devices |
| W13 | Measure Windows performance | CLI `benchmark`, media smoke, process-tree resource sampling | Release startup, paused idle CPU/RSS, sustained preview, seek p50/p95, export speed, A/V drift and power on representative hardware. Mac results do not establish Windows performance |

## Recording/output contract

The native editor consumes a finalized video plus optional same-stem `.system.m4a`, `.mic.m4a` and `.webcam.mp4` companions, and `<video filename>.cursor.json` containing `{version:1,samples:[...]}`. Audio also recognizes `.wav`/`.webm` companions. Samples use milliseconds with paused time removed and normalized `cx`/`cy`, plus `interactionType` and `cursorType`.

All capture participants must use a common time origin and pause accounting. Report readiness after the first video frame is accepted. A camera/audio failure must still allow the screen recorder to finalize and surface any partial result explicitly. Keep failed work and rollback files. The shared `file_group::replace` handles successful output replacement, including stale-track removal; do not invent a second Windows save format.

## Windows validation matrix

- x64 first; ARM64 is a separate build/runtime/device compatibility target. Decide whether 32-bit is supported rather than assuming it.
- Display/window recording; two monitors with different DPI; move/resize/minimize/close the target; sleep/wake; target/display removal.
- All audio/camera combinations; device denied/unavailable/disconnected; long capture and pause/resume synchronization.
- Open, relink, save, save-as, undo/redo, snapshot recovery and recording recovery; Unicode, spaces, long paths, file URLs, read-only/locked destinations and insufficient disk space.
- Generated 30/60/120-fps, VFR, rotation, long-GOP and large-resolution sources; all active visual effects; MP4/GIF; SRT/VTT; missing media; cancel at each export stage; preserve an existing destination on failure.
- Model download with redirects, interruption, cancellation and bad/truncated response; local transcription with embedded/companion audio and multilingual speech.
- Keyboard-only use, IME/text entry, screen reader, high contrast, scaling, fullscreen HUD/tray, file associations and update/uninstall behavior.

## Shared work that must not be relabeled Windows-only

Incomplete translation coverage for new native labels, full interaction and pixel differential acceptance, long-media acceptance and the native release/update lifecycle remain cross-platform work. See [PARITY.md](PARITY.md) for the precise limitations. “Shared implementation exists” does not mean “Windows parity is complete.”

### Recorder entry point update

Keep the shared `RecordingLauncher` and the overlay-to-editor transitions. Windows must finalize tray/taskbar visibility, overlay placement on the appropriate monitor/work area, DPI/dragging, capture exclusion and focus behavior. The Mac accessory/regular activation policy is deliberately isolated; it is not evidence that Windows taskbar behavior is finished. Recordings use the system Videos folder by default when available, with the chosen folder persisted in shared preferences.
