# Existing Electron baseline

Verified 12 September 2026 in `/Users/subs/Personal/SubTake`.

- Origin: `git@github.com-personal:SubhanHabib/SubTake.git`.
- Branch: `main`; base commit: `7512ef1518657edc2caae595136821d27a47167d`.
- Checkout was clean before adding `spike/`. Production code and the root lockfile are unchanged.
- The initial workspace path had no commit or source files. The user corrected the location; all build/test/runtime evidence below comes from the corrected checkout.
- Root manifest still identifies the Recordly-derived app as `recordly` / Recordly 1.4.0.

| Dependency | Exact root lockfile version |
| --- | --- |
| Electron | 43.1.0 |
| React | 18.3.1 |
| Vite | 5.4.21 |
| Pixi | 8.14.0 |
| pixi-filters | 6.1.5 |
| mediabunny | 1.25.1 |

## Verified source boundaries

`electron/main.ts` (1,130 lines), `windows.ts` (1,105), and `preload.ts` (1,022) own substantial lifecycle/window/bridge behavior. There are 138 textual `ipcRenderer.invoke/on/send` matches in the preload, and 48 source files referencing `window.electronAPI`. These are coupling indicators, not unique IPC-method counts.

`VideoPlayback.tsx` (2,961 lines) does not directly reference `window.electronAPI`. Its asset helpers do. Its existing Pixi scene motion, video texture ownership, and zoom logic are reusable without replacing the compositor. The spike imports this component directly from production source.

`projectPersistence.ts` defines the existing version-2 `{version, videoPath, editor}` envelope and editor normalizer. The spike uses that format and imports the normalizer. Its small command reducer operates on that envelope, preserving unknown editor fields. It does not replace the production project library, backup policy, or recent-file state.

`modernVideoExporter.ts` (3,750 lines) directly uses native metadata probes, export sessions, temporary files, audio muxing, progress, cancellation, and hardware diagnostics through Electron. A generic `exportVideo()` name does not remove those migration obligations.

`electron/rendererServer.ts` already serves packaged frontend assets on loopback HTTP. Production local-media approval/range-serving and project persistence live in additional IPC modules. The spike has a separate, fixture-scoped service so it does not broaden production file access.

No existing MCP SDK integration, project command bus, or HyperFrames adapter was found in `src`, `electron`, or the lockfile. Those were requested new spike capabilities, not verified reusable modules already in this revision.

## Baseline validation

- Root `tsc --noEmit`: passed.
- Existing Vitest suite: **124 files, 1,110 tests passed**.
- Dependencies installed with `npm ci --ignore-scripts`: the lockfile is respected, but the full postinstall/native recording-helper toolchain was not run.
- A production installer and the full production Electron application were **not** built or performance-profiled. The running reference slice must not be described as a full-app performance baseline.

The baseline evidence supports retaining Electron as the production default while testing a smaller platform boundary. It does not establish an end-user memory, battery, or launch-time advantage for either framework.
