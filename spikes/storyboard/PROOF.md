# Agent workflow integration evidence — 2026-09-20

The second workflow now uses the real HyperFrames 0.8.55 runtime. SubTake's normal
recording/native editing workflow is separate.

Verified in this run:

- User approved the three-scene test story in chat before capture.
- The agent navigated the real SubTake sample window through Computer Use while
  the new native capture CLI recorded that selected window for 35 seconds.
- Source frames show Settings, Cursor and Webcam panels. Original footage and
  cursor sidecars are preserved in `workspaces/agent-capture/`.
- Three four-second ranges became a 12-second HyperFrames composition. Its source
  references the captured MP4 directly; no intermediate assembly movie was made.
- HyperFrames check passed after resolving structural warnings: zero lint errors
  or warnings, zero runtime/layout/motion findings, and passing contrast checks.
- Each scene midpoint was inspected using HyperFrames snapshots.
- Actual WKWebView inside the SubTake process loaded both Studio and the
  lightweight HyperFrames player. Playback reached 12 seconds; scrubbing back to
  6 seconds displayed the cursor scene. Source changes hot-reloaded in Studio.
- The final app bundle was replaced and relaunched after a normal quit. No older
  app instance was left running. Final-build source enumeration still worked.
- Nine Python contract tests and ten native document/settings tests passed.
  The board's JavaScript parsed, `git diff --check` passed, and the packaged app
  passed the build script's codesign verification.

Current artifact: `workspaces/agent-feature-story/`. Its `verification.json`,
`embedded-preview.png`, `storyboard.json`, captured sources, and composition
snapshots are local evidence. Generated workspaces are intentionally gitignored.

Export is implemented with a reviewed-source fingerprint and an immutable copy,
then the actual HyperFrames check/render commands. Changed source or media is
rejected before rendering (contract-tested). Final sample export is awaiting
preview approval; a successful MP4 or preview/export parity is not claimed yet.

Limits: this remains a developer spike requiring this checkout and Node/Python;
Codex is the external agent. No built-in agent runtime, automatic agent wake-up,
TTS/music/source audio, generalized capture planner/retries, production packaging,
or storyboard/Studio edit round-trip. The sample contains test footage inside the
recorded app and is technical evidence, not finished promotional creative.
