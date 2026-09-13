# Electron / Tauri spike report

Recommendation: **keep Electron in production and continue a staged platform-interface refactor**. This isolated experiment does not justify a full Tauri migration. The report compares the same production preview component in two small shells; it is not a full-application benchmark.

The verified repository baseline is `7512ef1518657edc2caae595136821d27a47167d`, on `main`, in `/Users/subs/Personal/SubTake`. All additions are inside `spike/`; production source, styles, manifest and lockfile are unchanged. See [BASELINE.md](BASELINE.md).

## Test conditions and limits

Apple M5 Pro, 48 GiB RAM, macOS 27.0 / build 26A5421a, arm64; AC power, charging, Low Power Mode off. This is a developer machine in use, not a controlled battery/thermal lab. Root Electron is locked at 43.1.0; the built Tauri Rust crate is locked at 2.11.5; the Tauri JavaScript API/CLI are 2.10.1. FFmpeg is 9.0.1. No old-macOS/Windows/Linux claims follow from these results.

One shared Vite production bundle imports the existing 2,961-line `VideoPlayback` and existing project normalizer. Both shells open the same hashed ten-second 1080p30 H.264/AAC fixture. Both select Pixi WebGL in the measured path. The external Node test service, media/range route, dimensions, seek sequence and FFmpeg settings are shared. Native window creation is implemented separately in Electron and Rust.

Warm process-spawn timing excludes service startup and OS cold-cache/reboot effects. The first-preview measurement waits for readiness and a render submission; it is not a display/photon timestamp. The usable point includes eight cached thumbnails and runtime capability probes. Scripted scrub latency includes two RAF waits and an explicit Pixi render submission. Media-seek latency is reported separately. These synthetic metrics cannot prove continuous-drag or production-timeline parity.

## Active comparison

Five paired runs, alternating shell order, 40 deterministic seeks per run. Entries are medians of the five per-run metrics, not pooled p95s. The media and compiled-frontend hashes match across all ten runs. Raw data: [comparison.json](comparison.json).

| Metric | Electron | Tauri |
| --- | ---: | ---: |
| Warm spawn → first preview readiness/submission | 0.742 s | 0.891 s |
| Warm spawn → usable with thumbnail strip | 1.173 s | 1.588 s |
| Media seek p95 | 24.800 ms | 39.000 ms |
| Scripted seek + two RAFs + render p95 | 35.400 ms | 62.000 ms |
| Two-second diagnostic export | 1.955 s | 2.775 s |
| Project round-trip | 5/5 | 5/5 |
| Basic HUD open/close | 5/5 | 5/5 |
| WebGPU adapter / H.264 encoder capability probe | Available / supported | Available / supported |

Both actual preview paths use WebGL (`pixiRendererType: 1`). API availability probes do not prove a full WebGPU or WebCodecs export path. Electron leads these synthetic active metrics; this is preliminary evidence against assuming a Tauri speed-up, not a universal framework ranking.

## Ten-minute idle observations

Both sequential observations completed: Electron 600.0 seconds and Tauri 601.8 seconds, 119 and 119 raw samples respectively. [Idle summary](idle-summary.json).

| Diagnostic only: last-minute median | Electron | Tauri |
| --- | ---: | ---: |
| Tracked process-tree RSS, **incomplete accounting** | 1027.2 MiB | 223.0 MiB |
| Sum of tracked processes' `ps` CPU estimates, **incomplete for Tauri** | 18.9% of a core | 5.8% of a core |
| WebKit processes outside tracked tree | 7 | 10 |

**Do not compute a memory/CPU improvement from this table.** The Tauri WebKit subprocesses are not correctly attributed; some listed WebKit processes belong to other running applications. RSS also double-counts some shared pages and omits other resource ownership. The useful observation is that the paused Electron preview itself remains active; the controlled follow-up below investigates that.

The idle observations use an earlier equivalent preview build fingerprinted in `idle-build-manifest.json`. Its Electron launch includes the npm CLI wrapper; the final paired active runs launch Electron directly. Do not combine these as one perfectly controlled packaged-app experiment. Raw process lists include the common Node service and distinguish unattributed WebKit XPC processes. Parent/descendant RSS is not total physical application footprint and is not an appropriate Tauri memory-win claim.

## Paused-preview CPU diagnostic

In the final Electron slice, two 30-second process CPU-time intervals measured **20.3% of one core with normal paused rendering**, versus **5.1% with the existing preview-suspension flag enabled** (about 75% lower tracked CPU). Rendering was restored afterward. [Raw CPU-time deltas](paused-cpu-summary.json).

This is a single within-instance diagnostic, not a power/battery claim or a finished optimization. It supports prioritizing demand-driven paused rendering before changing shells. The residual CPU also means ticker suspension alone is not complete quiescence.

The source starts Pixi with `autoStart: true` (`VideoPlayback.tsx:576`). Its paused scene-update callback can return early (`:2132`), while Pixi's application ticker retains its own render callback. The component stops that ticker when `suspendRendering` is true (`:1304`); the production editor currently drives this flag from an in-progress modern MP4 export (`exportStatusModel.ts:62`). A production fix would need explicit invalidation and wake-on-seek/edit/play/resize/visibility behavior. The spike does not apply such a fix to production.

## Functional coverage

| Requested capability | Evidence / limit |
| --- | --- |
| Local video + real Pixi preview | Both shells pass; local file selected through fixture CLI argument |
| Timeline scrubbing | Same 40-request sequence; minimal range control, not the complete production timeline |
| Waveform or cached thumbnails | Eight in-memory cached preview thumbnails in each shell |
| Zoom or cursor effect | Production manual-zoom implementation is exercised |
| Project save/load | Same v2 project envelope, state round-trip passes; not the full project library/backup workflow |
| HUD boundary | Both native shells open/close one always-on-top window; broader window behavior remains untested |
| MCP | Real external stdio server and SDK client; shared renderer command bus, no UI click simulation |
| HyperFrames | Shared-project caption projects into a branded title card; validated and visually inspected, zero findings |
| Short export | Both produce a verified two-second 960×540 H.264 clip, 30 frames at 15 fps |
| Recording and full export pipeline | Not implemented or benchmarked in the spike |

The diagnostic exporter reads Pixi canvas frames and sends them to the same FFmpeg libx264 path. It is silent, rejects unsupported timeline/layer features, and does not exercise the production WebCodecs/native/FFmpeg exporter. Its duration is useful boundary evidence, not a claim about production export throughput.

Final SDK-client checks exercised all six MCP commands successfully in both shells. Ten `set_playhead` calls measured p95 round-trip latency of 33.6 ms in Electron and 41.8 ms in Tauri; these include seeking/render submission, not just transport overhead. Both returned valid preview PNGs. [MCP samples](mcp-summary.json).

MCP and HyperFrames remain independent of shell choice. MCP exposes `open_project`, `add_zoom_region`, `set_caption`, `set_playhead`, `render_preview`, and `export_video`. HyperFrames only projects the first caption into a title card; it does not serialize the entire SubTake timeline. The checked artifact is [artifacts/hyperframes/index.html](artifacts/hyperframes/index.html), with [preview](artifacts/hyperframes/preview.png) and [check evidence](artifacts/hyperframes/check.json).

## Metrics that remain unproven

| Metric | Status |
| --- | --- |
| True cold launch | Not measured; warm spawn/readiness figures only |
| Correctly attributed total idle physical memory | Not established; WebKit XPC/process-sharing attribution missing |
| Scrub GPU use / battery / energy | Not measured |
| Physical input-to-presented-frame p95 | Not measured; synthetic seek + paint proxy only |
| Dropped presented preview frames | Not comparable from seek-only counters; WebKit reports zero total frames |
| Recording CPU, frame/audio integrity | Not measured |
| Production export performance/quality | Not measured |
| Signed installer/download size | Not built; no size-based winner declared |
| Full HUD, capture permissions, native helpers | Only one basic window boundary tested |
| Minimum-supported macOS, Windows, Linux | Not tested |

## Validation

- Existing production typecheck passes.
- Existing Vitest suite: 124 files / 1,110 tests pass.
- Spike typecheck, build and focused lint pass.
- Eight command/transport/persistence tests pass.
- Tauri release binary builds with its committed lockfile.
- Both diagnostic exports verified with FFprobe: H.264, 960×540, 15 fps, 30 frames, 2.000 seconds.
- HyperFrames 0.8.35 check passes with zero lint/runtime findings; output snapshot inspected.

## Decision

Electron remains the production choice because this spike has not established a customer-visible Tauri resource win with editing/recording/export parity. Shared rendering and command architecture are feasible; full migration benefits remain unproven. First investigate demand-driven paused preview rendering, then extract production media/persistence boundaries and run the real native recording/export paths under both shells.

See [DECISION-GATES.md](DECISION-GATES.md) for proposed quantitative thresholds, platform coverage, and the next controlled experiment, and [README.md](README.md) for exact reproduction commands. Tauri's OS-dependent renderer behavior is documented in its official [webview reference](https://v2.tauri.app/reference/webview-versions/); feature checks on this Mac do not establish compatibility on the rest of the support matrix.
