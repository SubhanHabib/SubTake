//! The More card: the recent projects, the ways into the editor, and how a
//! recording is made — its resolution, frame rate, folder and the bar
//! while it runs.

use super::parts::{CardRow, group, key_caps, section_label};
use super::*;
use crate::ui_state::Recent;
use subtake_theme::FONT_MONO;
use subtake_ui::{edge, segmented};

#[cfg(test)]
mod tests;

/// The recent projects, three across.
const RECENT_COLUMNS: u16 = 3;
/// Resolution's choices: the setting each stores, its label, and its height
/// in lines (none for Native).
const RESOLUTIONS: [(&str, &str, Option<u32>); 5] = [
    ("native", "Native", None),
    ("2160", "2160p", Some(2160)),
    ("1440", "1440p", Some(1440)),
    ("1080", "1080p", Some(1080)),
    ("720", "720p", Some(720)),
];
const FRAME_RATES: [&str; 2] = ["30", "60"];

impl RootView {
    pub(super) fn more_card(
        &mut self,
        state: &RecordingOptions,
        cx: &mut Context<Self>,
    ) -> Vec<AnyElement> {
        let theme = self.theme;
        let mut body = Vec::new();

        // With nothing recent, the section goes.
        let recents: Vec<Recent> = state.get_recents().iter().collect();
        if !recents.is_empty() {
            body.push(
                section_label("Recent", theme)
                    .child(link("see-all", "See all", theme).on_click(self.command("projects")))
                    .into_any_element(),
            );
            body.push(
                div()
                    .grid()
                    .grid_cols(RECENT_COLUMNS)
                    .gap(px(Theme::recent_grid_gap()))
                    .children(recents.iter().enumerate().map(|(index, recent)| {
                        recent_tile(index, recent, theme).on_click(self.command(&recent.key))
                    }))
                    .into_any_element(),
            );
        }

        body.push(
            group(
                vec![
                    CardRow::new("more-open", "Open video or project…")
                        .plate("FolderOpen-regular")
                        .trailing(key_caps(&["⌘", "O"], theme))
                        .on_click(self.command("open")),
                    CardRow::new("more-projects", "All projects")
                        .plate("Stack-regular")
                        .trailing(
                            mono(state.get_project_count().to_string())
                                .text_size(px(Theme::font_secondary()))
                                .text_color(theme.muted),
                        )
                        .on_click(self.command("projects")),
                    // Not drawn by the design: the storyboard spike. It is a
                    // development entry point, so it stays last rather than
                    // being dropped while it is in use.
                    CardRow::new("more-storyboard", "Create video · spike")
                        .plate("Sparkle-regular")
                        .on_click(self.command("storyboard-spike")),
                ],
                theme,
            )
            .into_any_element(),
        );

        body.push(section_label("Recording", theme).into_any_element());
        let resolution = self.resolution_menu(state, cx);
        let frame_rate = {
            let options = state.clone();
            let current = state.get_recorder_setting("frame-rate");
            div()
                .flex()
                .flex_col()
                .w(px(Theme::frame_rate_width()))
                .child(segmented(
                    "frame-rate",
                    FRAME_RATES.len(),
                    FRAME_RATES.iter().position(|r| *r == current).unwrap_or(1),
                    Theme::card_segmented_height(),
                    theme,
                    move |index, _, _| {
                        options.defer_option("frame-rate".into(), FRAME_RATES[index].into())
                    },
                    |index, _| FRAME_RATES[index].into_any_element(),
                ))
        };
        let folder = icon_button(
            "save-to-folder",
            "FolderOpen-regular",
            "Choose folder",
            theme,
        )
        .small()
        .edged()
        .glyph_size(Theme::icon_size_card())
        .on_click(self.command("recording-folder"));
        let hide_bar = {
            let options = state.clone();
            switch(
                "hide-bar-toggle",
                state.get_recorder_flag("hide-bar"),
                true,
                theme,
                move |on, _, _| options.defer_option("hide-bar".into(), on.to_string()),
            )
        };
        body.push(
            group(
                vec![
                    CardRow::new("resolution", "Resolution").trailing(resolution),
                    CardRow::new("frame-rate-row", "Frame rate").trailing(frame_rate),
                    CardRow::new("save-to", "Save to")
                        .subtitle(middle_ellipsis(
                            &home_relative(&state.get_directory()),
                            Theme::save_path_chars() as usize,
                        ))
                        .subtitle_mono()
                        .trailing(folder),
                    // Not wired: the setting is kept, but the bar stays whole
                    // while recording.
                    CardRow::new("hide-bar", "Hide bar while recording").trailing(hide_bar),
                ],
                theme,
            )
            .into_any_element(),
        );

        // The global record / stop binding, as the keymap has it; with none
        // bound, no hint.
        let keys = shortcut_keys(&state.get_record_shortcut());
        if !keys.is_empty() {
            let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
            body.push(
                row()
                    .flex_none()
                    .gap(px(Theme::key_hint_gap()))
                    .px(px(Theme::card_text_inset()))
                    .text_size(px(Theme::font_secondary()))
                    .text_color(theme.muted)
                    .child(key_caps(&keys, theme))
                    .child("starts and stops recording from anywhere")
                    .into_any_element(),
            );
        }
        body
    }

    /// Resolution: the value and a caret at the row's end, opening the
    /// sizes no larger than the source. A display knows its height; a
    /// window does not, so it is offered every size, and one smaller than
    /// the size chosen records at its own.
    fn resolution_menu(&mut self, state: &RecordingOptions, cx: &mut Context<Self>) -> AnyElement {
        let native = native_height(state);
        let choices: Vec<_> = RESOLUTIONS
            .iter()
            .filter(|(_, _, lines)| match (lines, native) {
                (Some(lines), Some(native)) => *lines < native,
                _ => true,
            })
            .collect();
        let labels = choices
            .iter()
            .map(|(_, label, lines)| match (lines, native) {
                (None, Some(native)) => format!("{label} · {native}p"),
                _ => (*label).to_owned(),
            })
            .collect();
        let current = state.get_recorder_setting("resolution");
        let selected = choices
            .iter()
            .position(|(value, _, _)| *value == current)
            .unwrap_or(0);
        let values: Vec<&'static str> = choices.iter().map(|(value, _, _)| *value).collect();
        let options = state.clone();
        let menu = self.dropdown(
            "resolution",
            labels,
            selected as i32,
            true,
            cx,
            move |index, _, _| {
                if let Some(value) = values.get(index) {
                    options.defer_option("resolution".into(), (*value).into());
                }
            },
        );
        menu.update(cx, |d, _| d.bare = true);
        menu.into_any_element()
    }
}

/// A link at a section label's end: "See all".
fn link(id: &'static str, text: &'static str, theme: Theme) -> Stateful<Div> {
    let id = ElementId::from(id);
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let el = div()
        .id(id)
        .flex_none()
        .text_size(px(Theme::font_secondary()))
        .font_weight(FontWeight::NORMAL)
        .text_color(subtake_ui::motion::hover_blend(
            &hover_key,
            theme.text,
            theme.muted,
        ))
        .child(text);
    subtake_ui::pressable(el, theme, None, hover_key)
}

/// One recent project: its first frame at 16:10, its name, and its length
/// and age in mono under it.
///
/// Not drawn by the design: the handoff's picture is the project's first
/// frame; the library's still is what stands in, and until it is taken the
/// film strip does.
fn recent_tile(index: usize, recent: &Recent, theme: Theme) -> Stateful<Div> {
    let id = ElementId::from(SharedString::from(format!("recent-{index}")));
    let hover_key = subtake_ui::motion::tween_key(&id, "hover");
    let radius = Theme::recent_picture_radius();
    let picture = div()
        .relative()
        .w_full()
        .aspect_ratio(Theme::source_picture_aspect())
        .rounded(px(radius))
        .overflow_hidden()
        .bg(theme.sunk);
    let picture = match &recent.thumbnail.0 {
        Some(image) => picture.child(
            img(image.clone())
                .absolute()
                .inset_0()
                .size_full()
                .rounded(px(radius))
                .object_fit(ObjectFit::Cover),
        ),
        None => picture
            .flex()
            .items_center()
            .justify_center()
            .child(icon_sized(
                "FilmStrip-regular",
                Theme::icon_size_large(),
                theme.muted,
            )),
    }
    .child(edge(
        radius,
        vec![hairline(
            subtake_ui::motion::hover_blend(&hover_key, theme.line, theme.muted),
            Theme::hairline_width(),
        )],
    ));
    let tile = div()
        .id(id)
        .flex()
        .flex_col()
        .min_w_0()
        .gap(px(Theme::recent_tile_gap()))
        .child(picture)
        .child(
            div()
                .flex()
                .flex_col()
                .min_w_0()
                .px(px(Theme::source_caption_inset()))
                .gap(px(Theme::source_caption_gap()))
                .child(
                    div()
                        .text_size(px(Theme::font_secondary()))
                        .font_weight(FontWeight::MEDIUM)
                        .text_ellipsis()
                        .child(recent.title.clone()),
                )
                .child(
                    div()
                        .font_family(FONT_MONO)
                        .text_size(px(Theme::font_tiny()))
                        .text_color(theme.muted)
                        .text_ellipsis()
                        .child(recent.meta.clone()),
                ),
        );
    subtake_ui::pressable(tile, theme, None, hover_key)
}

/// The chosen display's height in lines, read off its "3456 × 2234".
fn native_height(state: &RecordingOptions) -> Option<u32> {
    let source = usize::try_from(state.get_source_index())
        .ok()
        .and_then(|i| state.get_capture_sources().iter().nth(i))?;
    if source.kind != "display" {
        return None;
    }
    source.detail.split('×').nth(1)?.trim().parse().ok()
}

/// A binding as the keymap spells it, "Super+Shift+R", as the caps the
/// Mac draws: ⌘ ⇧ R.
fn shortcut_keys(binding: &str) -> Vec<String> {
    binding
        .split('+')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(|part| {
            match part {
                "Super" | "Cmd" | "Command" | "Meta" | "CmdOrCtrl" => "⌘",
                "Shift" => "⇧",
                "Alt" | "Option" => "⌥",
                "Control" | "Ctrl" => "⌃",
                key => {
                    let key = key
                        .strip_prefix("Key")
                        .or_else(|| key.strip_prefix("Digit"))
                        .filter(|rest| !rest.is_empty())
                        .unwrap_or(key);
                    return key.to_uppercase();
                }
            }
            .to_owned()
        })
        .collect()
}

/// A path under the home folder from `~`.
fn home_relative(path: &str) -> String {
    match std::env::var("HOME") {
        Ok(home) if !home.is_empty() && path.starts_with(&home) => {
            format!("~{}", &path[home.len()..])
        }
        _ => path.to_owned(),
    }
}

/// `text` cut to `max` characters by taking out its middle, so a path
/// keeps both where it starts and the folder it ends in.
fn middle_ellipsis(text: &str, max: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max || max < 2 {
        return text.to_owned();
    }
    let keep = max - 1;
    let head = keep / 2;
    let tail = keep - head;
    chars[..head]
        .iter()
        .chain(['…'].iter())
        .chain(chars[chars.len() - tail..].iter())
        .collect()
}
