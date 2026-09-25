use super::{middle_ellipsis, shortcut_keys};

#[test]
fn shortcut_keys_draw_the_mac_caps() {
    assert_eq!(shortcut_keys("Super+Shift+R"), ["⌘", "⇧", "R"]);
    assert_eq!(shortcut_keys("Control+Alt+KeyP"), ["⌃", "⌥", "P"]);
    assert_eq!(shortcut_keys("Super+Shift+Digit2"), ["⌘", "⇧", "2"]);
    assert!(shortcut_keys("").is_empty());
}

#[test]
fn middle_ellipsis_keeps_both_ends() {
    assert_eq!(middle_ellipsis("~/Movies/SubTake", 38), "~/Movies/SubTake");
    let cut = middle_ellipsis("~/Library/Mobile Documents/com~apple~CloudDocs/SubTake", 20);
    assert_eq!(cut.chars().count(), 20);
    assert!(cut.starts_with("~/Library"));
    assert!(cut.ends_with("SubTake"));
    assert!(cut.contains('…'));
}
