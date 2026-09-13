# Decision gates and remaining experiment

## Recommendation at this stage

Keep Electron as the production shell. Continue the platform-interface work as a staged, reversible refactor. Do not authorize a full Tauri migration from installer size, generic framework benchmarks, or this reference slice alone.

The spike proves that the real Pixi preview and version-2 project envelope can be shared. It also proves a common external MCP command path and a shell-independent HyperFrames projection. The measured active path on the available Mac currently favors Electron. Full recording/export/HUD/permission parity and a trustworthy total-memory or battery win have not been established.

Tauri's renderer is WKWebView on macOS, WebView2 on Windows, and WebKitGTK on Linux. This makes the relevant test matrix OS/webview/codec-specific; macOS success on this machine is not evidence for older supported macOS versions. See the official [Tauri webview reference](https://v2.tauri.app/reference/webview-versions/).

## Proposed quantitative gates

These thresholds are proposed for the next controlled experiment, not retrospectively selected success criteria:

| Gate | Required evidence |
| --- | --- |
| Customer-visible resource win | At least 20% faster median true cold startup, or 25% lower correctly attributed steady physical memory, or 15% lower measured idle energy over a controlled interval |
| Interactive parity | No repeatable >5% p95 regression outside measurement uncertainty; also inspect continuous dragging, direction changes, dropped presented frames, and long-GOP worst cases |
| Export/recording parity | No repeatable >5% end-to-end slowdown or CPU regression at identical output quality; no frame/audio loss, sync regressions, or blocked codec paths |
| Reliability | Every critical capture permission, HUD, window lifecycle, native-helper, project round-trip, MCP, and HyperFrames case passes on every supported platform |
| Distribution | Compare signed/notarized installers with identical assets, FFmpeg, models, helper binaries, and any external runtime/service included |

A resource win cannot compensate for a failed core editing/recording/reliability gate. A median result cannot conceal a severe tail-latency or correctness regression.

## Next implementation stages

1. **Extract production media and persistence contracts.** Start with `src/lib/assetPath.ts`, `projectPersistence.resolveVideoUrl`, and the project lifecycle hooks. Retain current path approval, atomic saves/backups, resource disposal, and error/cancellation semantics. Leave rendering and styling unchanged.
2. **Replace spike service operations with real host adapters.** Electron delegates to existing IPC modules; Rust delegates to bounded filesystem/dialog/native-helper commands. Compare the full process set, including MCP and any retained Node sidecar. The existing spike `Platform` is a minimum proof, not a drop-in production API.
3. **Port one real recording/export path.** Bind the same macOS ScreenCaptureKit helper and the same production WebCodecs/native/FFmpeg path. Use typed session IDs, progress events, cancellation and structured errors. Do not route a Tauri export through a different quality preset just to make it faster.
4. **Prove production window behavior.** Repeat permission first-run/deny/grant/revoke, screen/window selection, microphone/system audio, HUD over fullscreen/Spaces, click-through/drag/focus, multi-monitor/DPI, countdown, minimize/restore, helper crash, and app quit. This spike tests only opening/closing one conventional always-on-top window.
5. **Measure packaged builds, then decide.** Keep the shared editor build/settings/media identical, alternate shell order, save raw traces, and report confidence intervals and failures. Stop migration work if a critical parity gate fails.

## Measurement protocol for the next controlled run

Use at least ten paired runs on each target laptop class and minimum-supported OS. Record model/chip/RAM, OS and exact webview/Electron version, AC/battery state, battery level, Low Power Mode, display refresh/resolution, external monitors, thermal state, and background workload. Use a fixed brightness and matched cache policy. True cold startup needs a defined cold-cache/reboot protocol; restarting a warm process is a separate metric.

Use the same local 1080p30 and 4K30/60 media plus relevant HEVC, variable-frame-rate, HDR, long-GOP, webcam/background-video, and audio cases. Record hashes, duration, codec/profile, frame rate, resolution, keyframe spacing and output settings. Freeze both frontend assets and backend adapters before sampling.

Measure three distinct interactive stages: command/input receipt, media-frame decode/seek completion, and actual composed-frame presentation. Sample continuous scrubbing as well as isolated seeks. Verify the frame timestamp matches the latest requested playhead; do not count superseded seeks as successful presentations. Do not equate two RAF waits with physical input-to-photon latency.

Measure 10 minutes of quiescent editor idle after equivalent activity and garbage-collection policy, then an active scrub/record session. Use an OS profiler that attributes WebKit XPC, Chromium helpers, GPU process, FFmpeg, native helpers and MCP/service processes correctly. Record physical footprint/shared-memory accounting, CPU time deltas and actual energy/GPU counters. Parent RSS is an incomplete diagnostic, not the resource-win gate. Battery percentage changes over a short interactive session are not a reliable energy metric.

For production exports, compare duration, frame count, visual correctness, audio sync, codec/profile/bitrate and quality. For MCP, distinguish state-command acknowledgment from a command that waits for rendering or export completion. HyperFrames success must include a checked composition and, when that render path is in scope, a verified render on the same external toolchain for both shells.
