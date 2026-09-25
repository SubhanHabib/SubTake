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

A second window, "SubTake components", opens beside the gallery. It lists every
primitive in `subtake_ui` in every state on one scrolling page: buttons,
pickers, sliders and fields, menus, tiles, status, rows and cards, the frosted
surfaces over a backdrop you pick in its header, the type scale and icons, and
the parked controls. Its header also has its own Light / Dark switch. Every
control is live. `SUBTAKE_GALLERY_COMPONENTS=off` leaves the window closed, and
`SUBTAKE_GALLERY_COMPONENTS=frost` (or any section name) opens it scrolled to
that section.

Each section's **Tune** button opens a dock listing every metric its
components read (padding, gaps, heights, radii, type and icon sizes), each
with a slider, nudges and a reset. A change is live in every window, the
editor's included, since a metric is one value app-wide, and a derived metric
follows the ones it is built from.

Once anything is tuned, a **Changes** column opens at the dock's right, one row
per change: the old and new value (a colour row shows both swatches and the new
hex), a copy button for that one `metrics.rs` or `palette.rs` line, and a
reset. **Copy all** puts every change on the clipboard, **Reset all** drops
them, and **Original / Tuned** flips every window between the shipped values and
the tuned ones without losing either. Clicking a colour row picks it in the
Colours view.

The dock's **Colours** view does the same for the palette on show: every colour
token as a chip, and the chosen one on hue, saturation, lightness and alpha
sliders and a field that takes any CSS colour. It lists the whole palette rather
than the section's tokens, since colour reads are not recorded. The copy carries
tuned colours as `palette.rs` lines.

The dock's **Themes** row keeps whole sets of changes to switch between. **Save
as…** writes what is tuned now to a `.subtaketheme` file, **Import…** loads one
from anywhere (and copies it in), and each saved theme is a chip that loads it
in place of what is tuned; the chip matching what is tuned now is lit. They live
in `~/Library/Application Support/com.SubTake.SubTake-Native/tuned-themes`. A
theme file is plain text, one `light.name=#hex`, `dark.name=#hex` or
`METRIC=value` per line, `#` for comments — the same entries
`SUBTAKE_GALLERY_TUNED` takes.

`SUBTAKE_GALLERY_COMPONENTS=buttons+tune` opens with the dock up (`+colours` in
its Colours view), and
`SUBTAKE_GALLERY_TUNED=GAP=12,RADIUS_MENU=8,dark.accent=#ff7a3d` starts with
those values tuned. `SUBTAKE_HOVER_PIN=play,btn-Transport` holds a hover on
for any control whose hover key contains one of the words, so a hover state
shows in a screenshot.

## Real-app walkthrough

The walkthrough is the gallery tour on the real app. It records the built-in
display with a pause, opens the take in the editor and fills every lane: zooms,
split clips, speed, trim, text, arrow and blur annotations, captions and a
music bed. It moves and trims a region, undoes and redoes, zooms and pans the
timeline and the picture, plays the edit back, visits every panel, crops,
opens Presets and exports, then quits. It needs no clicks, so it can be filmed.

```sh
SUBTAKE_WALKTHROUGH=light SUBTAKE_WALKTHROUGH_DIR="$PWD/test-output/walkthrough" python3 scripts/dev.py --once
```

Use `dark` for the dark appearance. The recording and `walkthrough-<theme>.mp4`
land in the directory. The run prints `WALKTHROUGH_PASSED` with the export's
size and length, or `WALKTHROUGH_FAILED` with the step and exits 1. It runs in
about 165 seconds.

The microphone and camera stay off, and settings and recovery are isolated, so
your preferences are untouched. Scrolls and pinches go to the window as real
input, through its hit testing; everything else fires the callbacks the
controls fire, so clicks and drags are not hit tested. Screen Recording
permission is needed,
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
