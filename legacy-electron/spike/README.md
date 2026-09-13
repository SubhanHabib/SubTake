# SubTake Electron / Tauri reference spike

One frontend imports the existing production `VideoPlayback` and project normalizer; two small window shells load the same built assets. This is an isolated macOS experiment. It does not replace the production application.

Read `BASELINE.md` for the verified checkout and `REPORT.md` for measured results and decision gates.

## Run from the repository root

```sh
npm ci --ignore-scripts
node node_modules/electron/install.js
npm ci --prefix spike
node spike/build.mjs
cargo build --release --locked --manifest-path spike/src-tauri/Cargo.toml
node spike/run.mjs electron --auto
# Close that instance before comparing the other shell.
node spike/run.mjs tauri --auto
```

The runner creates one deterministic 10-second 1080p/30 H.264/AAC fixture with a two-second GOP using local FFmpeg. Its hash is recorded. To use your own local MP4/WebM, pass `--media=/absolute/path/video.mp4`; use a source at least 8 seconds long for the automatic scenario. No media is uploaded. The fixture path is the only media file authorized by that service instance.

`--auto` runs preview, cached thumbnail capture, one zoom, 40 deterministic seeks, project round-trip, HUD open/close, diagnostic export, and title-card projection. `--exit` closes the shell after completion. `--idle=600` additionally waits/samples for ten minutes after active work. Run shells sequentially. The default run remains open for inspection.

```sh
node spike/run.mjs electron --auto --idle=600 --exit
node spike/run.mjs tauri --auto --idle=600 --exit
```

This launcher currently targets macOS binaries and uses `sw_vers` and macOS `ps`. Windows/Linux porting is an explicit outstanding gate, not an implied success.

## Boundaries

```text
Existing VideoPlayback + existing project format/normalizer
                  |
         Shared project command bus
          /                      React test controls          External MCP stdio server
                  |
           Platform contract
       /                         Electron adapter              Tauri adapter
Electron HUD IPC              Rust HUD command
       \                         /
     Shared fixture-scoped Node test service
      media ranges / persistence / FFmpeg
```

The renderer calls `Platform` methods (`openVideo`, `getLocalMediaUrl`, `saveProject`, `loadProject`, `showHud`, `exportVideo`). There are no new direct `window.electronAPI` calls. Recording returns an explicit unsupported error.

**The common Node service is test infrastructure.** Include its process in resource accounting; do not present these results as a pure Rust Tauri backend or a measurement of eliminating Node. A migration would replace the service behind the same contracts and retest native permissions and helpers. MCP stays outside the renderer and reaches project state through commands. The renderer owns the live project state in this slice; MCP does not own a second copy.

Local media selection is a CLI fixture path, persistence is a fixture-scoped JSON file, the thumbnail strip is cached in renderer memory, and the timeline control is a small range input. These prove boundaries; they do not reproduce all production UI behavior.

The title-card adapter consumes the current project's first caption text and emits a two-second branded HyperFrames composition. It is a narrow projection, not a faithful full-timeline conversion. A validated example and snapshots are under `artifacts/hyperframes`.

The two-second export is **30 Pixi canvas frames at 960×540 / 15 fps, encoded through FFmpeg libx264, without audio**. It exercises video/zoom rendering, canvas readback, transport, and encoding. It explicitly rejects captions, annotations, audio regions, webcam, trims, speeds, and clips instead of silently dropping them. It does not call the production `modernVideoExporter`.

## MCP

Each running shell writes `spike/results/<run>/connection.json` with a local ephemeral endpoint and token, mode 0600. Keep this ignored file local.

```sh
node spike/mcp.mjs /absolute/path/to/connection.json
node spike/mcp-check.mjs /absolute/path/to/connection.json
```

The first command is a real MCP stdio server for a client to launch; it reserves stdout for protocol messages. The check uses the official MCP SDK client, exercises the six listed commands, and stores latency samples plus a preview PNG.

| Command | Behavior |
| --- | --- |
| `open_project` | Load the saved fixture project |
| `add_zoom_region` | Validate and append a manual zoom region |
| `set_caption` | Set a validated caption in project state |
| `set_playhead` | Seek and wait for the reference render submission |
| `render_preview` | Return a PNG via MCP image content |
| `export_video` | Run the explicitly limited diagnostic export |

State-changing UI/MCP operations pass through one serialized command dispatcher. Scrub benchmarks are deterministic sequential commands, not continuous physical mouse drags. Production interactive coalescing, cancellation, undo, and multi-client revision conflict rules remain separate work.

## Validation and results

```sh
npm test --prefix spike
./node_modules/.bin/tsc -p spike/tsconfig.json --noEmit
npm test
```

`results/<run>/renderer.json` holds raw readiness/capability/seek/export data. `environment.json` records base commit, OS, media hash, PIDs, and timing limitations. `idle.json` records process-tree RSS and separately lists unattributed WebKit XPC processes. `idle-build-manifest.json` fingerprints the common compiled JavaScript used for the ten-minute observations; each final paired run records its frontend hash in `environment.json`. Root dependencies and spike dependencies each have their own lockfile; Cargo is locked separately.

**Measurement limits:** launch timing starts at process spawn with warm filesystem caches and an already-running service; it is not cold launch. First-preview timing is readiness plus render submission, not a photon/display timestamp. Scrub timing contains two RAF waits; `mediaSeekP95Ms` is recorded separately. Seek-only browser dropped-frame counters do not establish playback smoothness. macOS can reparent WebKit XPC processes, so descendant RSS must not be compared as total physical app memory. `ps` percent CPU is a sampled OS estimate, not isolated power usage. No installer, recording, GPU power, battery, minimum-supported-OS, Windows, or Linux result is implied.

## Paired active comparison and paused CPU diagnostic

```sh
node spike/compare.mjs
# Against a currently running Electron slice:
node spike/paused-cpu.mjs /absolute/path/to/results/run-directory
```

The comparison runs five pairs, alternating which shell starts first, and verifies media/frontend hashes match. It uses the Electron executable directly to avoid counting its npm CLI wrapper. The earlier ten-minute idle observation includes that wrapper in the raw process list and is labelled accordingly in the report.

The paused diagnostic compares two 30-second CPU-time intervals using the production component's existing `suspendRendering` prop. It restores normal rendering afterward. It is a diagnostic, not a production pause/wake implementation or a battery measurement.
