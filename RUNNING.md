# Running SubTake: the TL;DR

Every command runs from the repository root. The Python scripts wrap Cargo so
the native build tools (ninja, cmake for Skia) are found without any setup.

## Dev mode (rebuild on save)

```sh
python3 scripts/dev.py
```

Builds a debug binary, installs it to `target/dev/SubTake.app`, launches it and
watches `src/`, `crates/`, `scripts/`, `assets/` and the Cargo files. Saving a
source file rebuilds and relaunches. A failed build keeps the current app open.
Ctrl+C stops the watcher and asks the app to close.

| Want to…                              | Run                                  |
| ------------------------------------- | ------------------------------------ |
| Build and launch once, no watching    | `python3 scripts/dev.py --once`      |
| Stop a watcher running elsewhere      | `python3 scripts/dev.py --stop`      |

Only one dev watcher can run at a time; the lock lives in `target/dev`.

## Gallery in dev mode

The gallery opens every window with fixture data and touches no project. It is
the visual reference for controls and panels.

```sh
python3 scripts/dev.py --gallery
```

```sh
python3 scripts/dev.py --gallery light
```

The first starts in dark, the second in light. Inside the gallery, ⌘⇧D toggles
appearance, space plays, and the arrow keys nudge the playhead. Add `--once` to
skip watching. Without dev.py, any built binary opens the gallery with
`--gallery` as its first argument or `SUBTAKE_GALLERY=dark|light` in the
environment.

## Real-app walkthrough

The walkthrough is the gallery tour on the real app. It records the built-in
display, opens the take in the editor, visits every panel, adds a zoom, opens
Presets and exports, then quits. It needs no clicks, so it can be filmed.

```sh
SUBTAKE_WALKTHROUGH=light SUBTAKE_WALKTHROUGH_DIR="$PWD/test-output/walkthrough" python3 scripts/dev.py --once
```

Use `dark` for the dark appearance. The recording and `walkthrough-<theme>.mp4`
land in the directory. The run prints `WALKTHROUGH_PASSED` with the export's
size and length, or `WALKTHROUGH_FAILED` with the step and exits 1. It runs in
about 110 seconds.

The microphone and camera stay off, and settings and recovery are isolated, so
your preferences are untouched. It fires the callbacks the controls fire;
pointer hit testing is not exercised. Screen Recording permission is needed,
and the Mac must be unlocked: a locked screen records only the lock screen.

## Debug vs release

Debug is what dev mode builds: fast compile, slow code, debug assertions on.
Release is optimised and is what ships. Both produce the same `.app` layout.

| Mode    | Command                                   | Output                        |
| ------- | ----------------------------------------- | ----------------------------- |
| Debug   | `python3 scripts/build-mac.py`            | `dist/SubTake.app`            |
| Release | `python3 scripts/build-mac.py --release`  | `dist/SubTake.app`            |

The bundle in `dist/` is self-contained (resources copied, not linked) and
ad-hoc signed. Open it with:

```sh
open dist/SubTake.app
```

Developer ID signing and notarization are separate distribution steps; pass
`--identity "Developer ID Application: …"` to sign with a real identity.

## Plain Cargo

For a compile check or to run the binary against a file without any bundle:

```sh
cargo build --locked
```

```sh
cargo run -- /absolute/path/to/video.mp4
```

If Skia fails to find ninja, run Cargo through the shared environment instead:

```sh
python3 scripts/build_env.py build --locked
```

## Checks before a commit

```sh
cargo fmt --all -- --check && cargo check --all-targets && cargo test --all-targets && cargo clippy --all-targets && python3 scripts/check-ui-primitives.py && scripts/test-native.sh
```

Conventions for the code itself are in [docs/CONVENTIONS.md](docs/CONVENTIONS.md).
