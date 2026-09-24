# SubTake Rust conventions

This is the Rust profile of the house style described in SubNotes'
`docs/REACT-SCSS-STRUCTURE.md`: domain-oriented folders, explicit façades,
names that say what a thing holds, shallow control flow, and whitespace owned
by the formatter. Where Rust already has a strong convention this document
follows Rust, not the TypeScript spelling.

The formatter (`rustfmt.toml`) and Clippy enforce the mechanical parts. The
rest is judgement, and this document is the reference for that judgement.

## Principles

- **One file, one responsibility.** A file is named for the one thing it owns
  and a reader should be able to guess the file from the symbol.
- **Façade plus folder.** A module that outgrows one file becomes `foo.rs` next
  to a `foo/` directory. The façade declares the children, re-exports what the
  rest of the crate needs, and holds the shared types and state. Children start
  with `use super::*;` and mark what siblings need `pub(super)`. No `mod.rs`.
  The trigger is "several things in one file", not a line count: a single
  component or a single `match` stays in one file however long it is.
- **Names carry meaning; the formatter carries layout.** Nothing is aligned by
  hand, and no name is shortened to a letter unless it is in the vocabulary
  table below.
- **Behaviour is the contract.** Structural and naming changes never alter
  labels, project-file keys, callbacks, keyboard handling or undo/redo. Each
  such change is its own commit with a message that says so.

## Project structure

```text
Cargo.toml                 # workspace: the app plus two presentation crates
rustfmt.toml               # formatter profile, stable options only
src/
├── main.rs                # argument parsing and process entry only
├── lib.rs                 # the crate's public surface
├── app.rs                 # App state, STATE thread-local, post/with_app
├── app/
│   ├── actions.rs         # every "action" string and add_region
│   ├── documents.rs       # library, load, save, recovery
│   ├── fields.rs          # inspector field lists, nested JSON get/set
│   ├── playback.rs        # preview decoder thread, seek, refresh
│   ├── recorder.rs        # launcher and options windows, hotkeys, tray
│   ├── run.rs             # run(): event loop, macOS glue, status menu
│   ├── smoke.rs           # scripted UI smoke run
│   └── smoke/tests.rs     # its tests
├── ui.rs                  # RootView, Surface, gestures, preview geometry
├── ui/
│   ├── editor.rs          # editor window chrome and brand
│   ├── inspector.rs       # rail, panel buttons, field rows
│   ├── menus.rs           # menu bar, palette, command dispatch
│   ├── preview.rs         # preview canvas, pan, magnify, cursor image
│   ├── recorder.rs        # launcher bar and options sheet
│   └── timeline.rs        # timeline rows, seek and drag gestures
├── ui_state.rs            # window property/callback surface (surface! macro)
├── ui_runtime.rs          # shared runtime state and the public surface
├── ui_runtime/            # models, weak, dispatch, timer, window, event_loop, menus, assets, tests
├── gallery.rs             # --gallery: the windows, echo callbacks and Gallery state
├── gallery/               # fixtures.rs (fake data), images.rs (generated imagery)
├── render.rs              # Scene and Scene::render
├── render/                # captions.rs, cursor.rs, backend.rs
├── platform.rs            # helper lookup, sources, devices, reveal
├── platform/              # recording.rs, companion.rs, windows.rs (unsafe FFI)
└── <domain>.rs            # project, editing, export, timeline, …
native/                    # in-process macOS code, Swift only, one concern per file
├── RecorderGlass.swift    # frosted material and plate masks
├── RecorderWindows.swift  # recorder bar, options card, countdown
├── RuntimeWindow.swift    # show, hide, focus, position, drag
├── …                      # Magnify, StatusItem, DocumentEvents, AgentWorkspace, BrandMark
└── tests/                 # run by scripts/test-native.sh
scripts/*.swift            # helper executables the app spawns (capture, devices)
crates/
├── theme/src/
│   ├── lib.rs             # Theme struct, constructors, derived colours
│   ├── appearance.rs      # Appearance and preference resolution
│   ├── css.rs             # css("hsla(…)") token parser
│   ├── palette.rs         # the light and dark colour tokens
│   ├── metrics.rs         # sizes, radii, type scale, fonts, layout columns
│   └── tests.rs
└── ui/
    ├── src/lib.rs         # façade: pub use of every control and helper
    ├── src/controls.rs    # declares one child per component
    ├── src/controls/      # button.rs, dropdown.rs, slider.rs, switch.rs, …
    ├── src/frost.rs       # vibrancy plates and fades
    ├── src/icon.rs        # icon() and icon_sized()
    ├── src/layout.rs      # row(), column(), measure()
    ├── src/motion.rs      # tweens and hover fades
    └── src/perf.rs        # frame counters
```

Placement rules:

- A generic control (`Button`, `Slider`, anything with no knowledge of
  projects or recording) lives in `crates/ui/src/controls/<name>.rs`, one
  component per file, and is re-exported flat from `crates/ui/src/lib.rs`.
  `scripts/check-ui-primitives.py` fails the build if one appears elsewhere.
- Colours, sizes and fonts are tokens on `Theme` in `crates/theme`. A bare
  number where a token belongs is a lint failure.
- Application presentation (anything that reads `EditorWindow` or `App`) lives
  under `src/ui/`. Application state and behaviour live under `src/app/`.
- Domain logic that is independent of the window (project format, editing
  operations, rendering, export) is a top-level `src/<domain>.rs` and has no
  `gpui` import.
- Children use `use super::*;` and mark cross-file items `pub(super)`. The
  façade imports whatever the children share so that one line is enough.

## Naming

| Construct                                    | Convention             | Examples                                |
| -------------------------------------------- | ---------------------- | --------------------------------------- |
| Crates, modules, files, functions, variables | `snake_case`           | `caption_editing`, `snap_delta`         |
| Structs, enums, traits, variants             | `UpperCamelCase`       | `EditorWindow`, `TimerMode::SingleShot` |
| Constants and statics                        | `SCREAMING_SNAKE_CASE` | `TITLEBAR_HEIGHT`, `HOVER_FADE_MS`      |
| Feature flags                                | kebab-case             | `native-menu`                           |

- **Parameters and locals are named for what they hold**, not for their type's
  initial: `project`, `theme`, `window`, `value`, `text`, `time`, `width`.
  The closures handed to `post` and `with_app` take `|app, ui|`.
- **Predicates read as questions or states**: `is_playing`, `has_selection`,
  `can_replace`, `visible`. Avoid `flag` or `ok` as names.
- **Converters follow cost**: `as_` borrows for free, `to_` copies, `into_`
  consumes.
- **Traits name a capability** (`Appearance`, `Recorder`), never `FooTrait` or
  `FooInterface`.
- **Privacy is the default.** Items are private, `pub(super)` when a sibling
  file needs them, `pub(crate)` when the crate does, and plain `pub` only on
  the crate's real surface. No `_` prefix on private files or functions; a
  leading underscore means "intentionally unused".
- **Serialized names are frozen.** JSON keys such as `zoomRegions` and
  `cropRegion` and action strings such as `"split-clip"` are the project
  format. They are quoted strings, not identifiers, and a rename pass never
  touches them.

### Short names we keep

Some names are short because the whole ecosystem spells them that way, and a
longer spelling would read as foreign. These are the only single- or
two-letter names allowed in SubTake:

| Name       | Meaning                                              | Where                       |
| ---------- | ---------------------------------------------------- | --------------------------- |
| `cx`       | GPUI `App` or `Context<Self>` handle                 | every render and handler    |
| `ui`       | the `EditorWindow` passed alongside `app`            | `post`, `with_app` closures |
| `px`       | GPUI's pixel constructor                             | everywhere                  |
| `dt`       | elapsed time between two frames, seconds             | motion and playback         |
| `id`       | an identifier of any kind                            | regions, documents          |
| `x`, `y`   | coordinates                                          | geometry                    |
| `w`, `h`   | only inside a tight geometry expression, never a parameter | geometry              |
| `i`, `n`   | loop index and count in a loop of a few lines        | loops                       |
| `t`        | a normalised 0–1 position inside an easing function  | `ease_*`, `lerp`            |
| `a`, `b`   | the two ends of an interpolation                     | `lerp`, `sample`            |
| `r`, `g`, `b` | colour channels inside a colour constructor       | `Color`, gradients          |

Anything else gets a word.

## Whitespace and line breaks

`cargo fmt` owns wrapping and indentation: four spaces, 100 columns, 2024
style edition. Blank lines are yours, and they mark **semantic paragraphs**:
acquire input, derive, apply, return. Do not blank-line every statement, and do
not run two phases together.

**Every item is separated by one blank line**: between two functions, between a
function and the `impl` or `}` around it, between a struct and its `impl`,
between consts, and before every `#[…]` attribute that opens a new item. Two
adjacent closing braces are fine; a `}` directly followed by `fn` is not.
`rustfmt` keeps blank lines it finds but does not insert them, so this one is
checked by eye and in review.

```rust
pub(super) fn load(&mut self, ui: &EditorWindow, path: &Path) -> Result<()> {
    let text = std::fs::read_to_string(path).with_context(|| path.display().to_string())?;
    let project = Project::parse(&text)?;

    self.history = Some(History::new(project));
    self.path = Some(path.to_owned());

    self.refresh(ui);
    Ok(())
}
```

Within a file:

1. doc comment for the module, if the file's name alone is not enough;
2. imports, grouped as standard library, third-party, then `crate::`/`super::`
   with one blank line between groups;
3. constants and type definitions;
4. the primary public API;
5. `impl` blocks;
6. private helpers;
7. `#[cfg(test)] mod tests;` as the last line.

Tests live in a sibling file, never inline. A module `foo.rs` declares
`#[cfg(test)] mod tests;` and keeps the bodies in `foo/tests.rs`, which starts
with `use super::*;` and so sees the module's private items without anything
being made public for the test's sake. A crate root does the same with
`src/tests.rs`.

Method chains that express a sequence of transforms break vertically. GPUI
element builders are such chains: one `.child()`, `.on_click()` or style call
per line once the chain has more than two links. Short closures stay inline; a
closure with more than one conceptual step gets a block.

## Control flow and abstraction

- Prefer early `return`, `let … else` and `?` over nested `if let`.
- Use `match` exhaustively over enums. Model states as enums, not clusters of
  booleans (`Style::Preview | Strip | Waveform`, not three flags).
- `Option` for absence, `Result` for failure. At the application boundary add
  context with `anyhow::Context`; inside a reusable module return a typed error
  when callers branch on it.
- No `unwrap()` or `expect()` on a runtime path. They are fine in tests, in the
  gallery fixtures, and for a locally obvious invariant with a comment saying
  why it holds.
- Borrow at read-only boundaries (`&Project`, `&str`, slices). Clone when a
  closure or thread genuinely needs ownership, and say so if it is not obvious.
- Add a trait when there are two implementations or a test double, never one
  trait per struct.
- No `utils` module. A helper lives next to its only caller; a shared concept
  gets a precisely named module.
- `unsafe` stays under `src/platform/` and `src/ui_runtime/` behind small
  safe functions, each with a `// SAFETY:` comment.

## Native macOS code

**No more Objective-C. Native macOS code is Swift, and only Swift.** That
covers anything that touches AppKit, WebKit, Core Animation or any other Apple
framework, whether it runs inside the app or as a helper. Do not add `.m`,
`.mm` or Objective-C headers, and do not reach for `objc2` from Rust to avoid
writing Swift. Portable C that is not macOS-specific, such as the FFmpeg
decoder in `scripts/decoder.c`, is not native macOS code and stays C.

- **In-process code** lives in `native/`, one concern per file, and
  `build.rs` compiles every `native/*.swift` into one static library. Each
  entry point is a `public` function marked `@_cdecl("subtake_…")` and is
  declared in an `extern "C"` block in `src/platform/`. Keep the two
  signatures identical: `UnsafeMutableRawPointer?` for `*mut c_void`,
  `Bool` for `bool`, `Double` for `f64`, `UInt` for `usize`, and a
  `@convention(c)` typealias for every callback.
- **Helper executables** stay as single Swift files in `scripts/`, built by
  `dev.py` and `build-mac.py`.
- GPUI owns every view it hands over. Borrow it with `borrowedView(_:)`, never
  retain it, and keep per-view state in an associated object keyed with
  `associationKey()` so it dies with the view.
- AppKit calls run on the main thread. Every entry point that mutates a window
  starts with `assert(Thread.isMainThread, …)`.
- Objective-C ignored messages to nil, and Swift traps on them. Reach the
  application as `NSApp?.`, never `NSApp!` or `NSApplication.shared`, which
  creates the application early and pre-empts GPUI. Library code has no force
  unwraps. Return early with `guard let` instead.
- The library builds in Swift 5 language mode. Moving to Swift 6 strict
  concurrency is a separate change, not something to half-do in a feature.
- Tests go in `native/tests/` and run with `scripts/test-native.sh`.

## GPUI specifics

- Rendering is immediate-mode. State that drives a frame lives on the view; a
  draw that needs another frame calls `request_animation_frame`, never
  `cx.notify()` from inside the draw.
- Window property setters and callbacks are generated by the `surface!` macro
  in `ui_state.rs`. Add a property there, not by hand on one window.
- Every control takes its colours from `Theme` and its sizes from
  `Theme::FONT_*` and `Theme::ICON_SIZE*`.
- Read an `f32` metric through its generated reader, `Theme::gap_small()`
  for `Theme::GAP_SMALL`, so the component catalogue can tune it live. The
  constant is for const contexts, tests and `src/ui_runtime/window.rs` only.
- The gallery (`--gallery` or `SUBTAKE_GALLERY=light|dark`) is the visual
  reference for every window and control. A presentation change is checked
  there in both appearances before it is called finished.

## Tooling and validation

Run these as separate checks. All of them pass on every commit.

```sh
cargo fmt --all -- --check
cargo check --all-targets
cargo test --all-targets
cargo clippy --all-targets
python3 scripts/check-ui-primitives.py
scripts/test-native.sh
```

Clippy runs with its default lint set. Allow a lint narrowly, on the item, with
a comment saying why. Do not add restriction lints wholesale.

## Migration rules

- A restructuring commit is moves and renames only. It changes no public API,
  no serialized key, no label and no behaviour, and its message says which.
- Formatting changes are their own commit so that a blame across a file lands
  on the author of the logic, not on `cargo fmt`.
- A rename pass is scoped to the binding's own block. Serialized strings and
  comments are checked afterwards, because a word-boundary search will happily
  rename the `s` in "user's".
