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
