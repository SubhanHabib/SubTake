//! The macOS menu bar and its keyboard shortcuts.

use super::*;

#[derive(Clone, Debug, PartialEq, serde::Deserialize, gpui::Action)]
#[action(no_json)]
pub(super) struct EditorCommand {
    action: String,
}
pub(super) fn install_menus(cx: &mut gpui::App) {
    cx.on_action(|command: &EditorCommand, _| {
        let command = command.action.clone();
        Timer::single_shot(Duration::ZERO, move || {
            let editor = SURFACES.with(|s| {
                s.borrow().iter().find_map(|s| {
                    if let Surface::Editor(ui) = s {
                        Some(ui.clone())
                    } else {
                        None
                    }
                })
            });
            if let Some(ui) = editor {
                // `@Panel` opens an inspector; everything else is an action.
                // The palette in `ui::menus` strips the prefix the same way.
                if let Some(panel) = command.strip_prefix('@') {
                    ui.set_panel(panel.into());
                    ui.invoke_panel_change(panel.into());
                } else {
                    ui.invoke_action(command);
                }
            }
        });
    });
    // Both menu bars are built from `ui::menu_commands`, so the
    // palette and the OS menu can no longer drift apart. Groups in that table
    // become the separators here.
    let menu = |name: &'static str| gpui::Menu {
        name: name.into(),
        items: crate::ui::menu_commands(name)
            .iter()
            .enumerate()
            .flat_map(|(group, commands)| {
                (group > 0)
                    .then(gpui::MenuItem::separator)
                    .into_iter()
                    .chain(commands.iter().map(|(label, command)| {
                        gpui::MenuItem::action(
                            (*label).to_owned(),
                            EditorCommand {
                                action: (*command).into(),
                            },
                        )
                    }))
            })
            .collect(),
        disabled: false,
    };
    let action = |label: &str, command: &str| {
        gpui::MenuItem::action(
            label.to_owned(),
            EditorCommand {
                action: command.into(),
            },
        )
    };
    cx.set_menus(vec![
        gpui::Menu {
            name: "SubTake".into(),
            items: vec![
                gpui::MenuItem::os_submenu("Services", gpui::SystemMenuType::Services),
                gpui::MenuItem::separator(),
                action("Quit SubTake", "quit"),
            ],
            disabled: false,
        },
        menu("File"),
        menu("Edit"),
        menu("Help"),
    ]);
    // The keymap is also what gpui reads to print a menu item's shortcut
    // column, so anything listed in the menus above has to be bound here or
    // it shows blank even though the keystroke works.
    //
    // Only the keys with no text-field meaning are bound. A key equivalent on
    // an NSMenuItem is consumed by Cocoa in `performKeyEquivalent:`, BEFORE
    // the window sees a key-down — so binding Copy/Cut/Paste/Select All here
    // would take them away from a focused `TextInput`, which has its own
    // scoped bindings for them. Those four stay on the key-down path in
    // `RootView`, which already steps aside when an input holds focus.
    let key = |keystroke: &str, command: &str| {
        gpui::KeyBinding::new(
            keystroke,
            EditorCommand {
                action: command.into(),
            },
            None,
        )
    };
    cx.bind_keys([
        key("cmd-q", "quit"),
        key("cmd-o", "open"),
        key("cmd-s", "save"),
        key("cmd-shift-s", "save-as"),
        key("cmd-z", "undo"),
        key("cmd-shift-z", "redo"),
    ]);
}
