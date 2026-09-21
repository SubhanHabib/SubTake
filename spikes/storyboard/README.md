# Agent storyboard spike

This is a working experiment: an existing coding agent supplies the reasoning and
visual inspection, while SubTake supplies deterministic media assembly and rendering.
There is no embedded LLM, account integration, HyperFrames dependency, or TTS service.
No HeyGen code was copied. The workflow is implemented for native SubTake.

## Try it with an agent

Give the agent [SKILL.md](SKILL.md), a feature brief, and screenshots or recordings.
For this checkout, paste:

> Read `/Users/subs/Personal/SubTake/spikes/storyboard/SKILL.md` and use the SubTake
> storyboard spike. Open the context for
> `/Users/subs/Personal/SubTake/spikes/storyboard/workspaces/demo`.
> Inspect the supplied media, read my feedback, and propose a stronger storyline.
> Update the board, then stop for my approval before generating a new draft.
> Preserve existing generations and any edits I made in the native editor.

The sample uses still screenshots with test footage visible in their editor previews.
It demonstrates the agent/board/native pipeline, not a polished marketing film.
The sample draft was generated during acceptance testing; its storyline still needs
the user's review. All narration fields are script text only.

## Entry points

- Native app: folder button → Projects → **Create video · spike**.
- Recorder: More → **Create video · spike**.
- CLI: `python3 spikes/storyboard/storyboard.py launch WORKSPACE`.

The native entry opens the last launched workspace in a local browser companion.
It does not automatically attach the currently open native recording.
The local service is bound to loopback and uses a per-run token. Closing the browser
does not stop it. Its PID and URL are in `WORKSPACE/.server.json`.

## Agent commands

```sh
python3 spikes/storyboard/storyboard.py init /absolute/new-workspace --brief /absolute/spec.txt --asset /absolute/screen.png --asset /absolute/recording.mp4
python3 spikes/storyboard/storyboard.py context /absolute/new-workspace
python3 spikes/storyboard/storyboard.py plan /absolute/new-workspace --file /absolute/plan.json --revision 0
python3 spikes/storyboard/storyboard.py launch /absolute/new-workspace
# After approval in the board:
python3 spikes/storyboard/storyboard.py build /absolute/new-workspace
python3 spikes/storyboard/storyboard.py open /absolute/new-workspace
```

An example plan is in [examples/plan.json](examples/plan.json). The CLI copies input
media into the workspace. The board polls for agent changes and exposes a feedback
channel, scene edits, and reordering. Feedback does not wake the agent automatically:
reply in its chat after submitting feedback. No MCP registration is needed; this
spike intentionally uses the same skill + CLI pattern as agent video workflows.

## What the draft contains

Each successful build has its own `builds/generation-*/` directory containing:

- `assembly.mp4`: supplied media in the approved scene order, without source audio.
- `draft.recordly`: native clip boundaries, editable zooms and captions.
- `preview.mp4` and `scene-*.png`: output from the native SubTake renderer.
- `report.json`: scene-to-source mapping, plan revision, renderer binary hash.

Multiple sources and arbitrary ordering work by assembling one base video. Native
timeline edits are not synced back into the storyboard. A new build never overwrites
an earlier draft; keep native edits in that generation or save them elsewhere.
The board is not a native Slint storyboard, does not autonomously navigate apps,
does not fetch PRs/URLs, and does not generate arbitrary motion graphics or voiceover.
The agent may gather authorised context separately, then use these commands.

## Checks

`python3 -m unittest discover -s spikes/storyboard -p 'test_*.py'`

Tests cover approval invalidation, stale writes, media bounds, feedback, preservation,
and scene identity. The end-to-end acceptance run also exercises browser approval,
feedback → agent revision → regeneration, playback, narrow layout, and native opening.
See the local `workspaces/demo/verification.json` for the actual run evidence.

## Actual HyperFrames / agent capture spike (2026-09-20)

The default for new CLI intake is now `--engine hyperframes`. The earlier native
backend remains available via `--engine native`; existing workspaces are preserved.

The agent uses the native `capture WINDOW_ID OUTPUT.mp4 SECONDS` CLI plus Computer
Use. It then imports the captured file and source ranges into the reviewed board.
Generate preview runs HyperFrames 0.8.55, checks the HTML composition and starts a
loopback Studio. Projects → Create video · spike opens the board and lightweight player (with an optional Studio timeline) inside
SubTake's WKWebView (separate workspace window, ephemeral browser data, no native
JavaScript bridge). There is no intermediate movie generation in this backend.

`npm ci --prefix spikes/storyboard` installs the pinned local runtime. Follow
[SKILL.md](SKILL.md) for the complete agent flow. `launch WORKSPACE --no-open`
selects which workspace the native entry opens. A live Studio generation is also
editable with HyperFrames directly; Refresh preview updates the reviewed hash.
Approve & export validates that hash, freezes the source and invokes HyperFrames'
renderer. The board has a resulting movie download link. Generated workspaces and
node_modules are ignored; package-lock.json pins the runtime.

The agent is still Codex (or another external agent), not embedded in SubTake.
Capture requires macOS screen recording access for the running build. This is a
local developer integration, not production packaging, an autonomous agent loop,
or a finished visual editor. Narration remains script-only and source audio is
not included. Native project edits and Studio edits do not sync back into the
storyboard; regeneration preserves earlier generations instead.

Stop a generation's managed Studio with:
`spikes/storyboard/node_modules/.bin/hyperframes preview WORKSPACE/builds/GENERATION/composition --stop`.
The board server is a separate local Python process recorded in `.server.json`.
