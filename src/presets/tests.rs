use super::*;
#[test]
fn removal_retains_bytes_and_rejects_outside_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("My preset.json");
    std::fs::write(&path, b"saved preset").unwrap();
    let retained = remove_from(dir.path(), &path).unwrap();
    assert!(!path.exists());
    assert_eq!(std::fs::read(&retained).unwrap(), b"saved preset");
    let other = tempfile::tempdir().unwrap();
    let outside = other.path().join("keep.json");
    std::fs::write(&outside, b"keep").unwrap();
    assert!(remove_from(dir.path(), &outside).is_err());
    assert!(outside.exists());
}

#[test]
fn every_key_is_in_one_part() {
    for key in KEYS {
        let parts = GROUPS
            .iter()
            .filter(|(_, _, keys)| keys.contains(key))
            .count();
        assert_eq!(parts, 1, "{key}");
    }
    for (_, _, keys) in GROUPS {
        for key in keys {
            assert!(KEYS.contains(key), "{key} is in a part but never saved");
        }
    }
}

fn project() -> Project {
    let mut project = Project::new(Path::new("take.mp4"));
    project.set("padding", json!(40));
    project.set("cursorSize", json!(3.));
    project
}

#[test]
fn a_part_turned_off_is_kept_but_not_applied() {
    let dir = tempfile::tempdir().unwrap();
    let path = create(dir.path(), &project(), &["look", "cursor"]).unwrap();
    assert_eq!(name(&path), "Preset");
    set_group(&path, "cursor", false).unwrap();
    let data = load(&path).unwrap();
    assert_eq!(groups(&data), ["look"]);
    assert_eq!(data["snapshot"]["cursorSize"], json!(3.));

    let mut target = Project::new(Path::new("other.mp4"));
    target.set("padding", json!(0));
    target.set("cursorSize", json!(1.));
    apply(&mut target, &data).unwrap();
    assert_eq!(target.editor["padding"], json!(40));
    assert_eq!(target.editor["cursorSize"], json!(1.));

    set_group(&path, "cursor", true).unwrap();
    assert_eq!(groups(&load(&path).unwrap()), ["look", "cursor"]);
    assert!(set_group(&path, "look", false).is_ok());
    assert!(set_group(&path, "cursor", false).is_err(), "last part");
}

#[test]
fn a_file_without_parts_applies_everything() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("Old.json");
    save(&path, &project()).unwrap();
    assert_eq!(groups(&load(&path).unwrap()).len(), GROUPS.len());
}

#[test]
fn names_count_up_and_are_never_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let first = create(dir.path(), &project(), &["look"]).unwrap();
    let second = create(dir.path(), &project(), &["look"]).unwrap();
    assert_eq!(name(&second), "Preset 2");
    assert!(rename(dir.path(), &second, "Preset").is_err());
    assert!(rename(dir.path(), &second, "  ").is_err());
    assert!(rename(dir.path(), &second, "a/b").is_err());
    let renamed = rename(dir.path(), &second, " Client demo ").unwrap();
    assert_eq!(name(&renamed), "Client demo");
    // Only the casing changes: the same file, not a clash.
    let recased = rename(dir.path(), &first, "PRESET").unwrap();
    assert_eq!(name(&recased), "PRESET");
    let copy = duplicate(dir.path(), &renamed).unwrap();
    assert_eq!(name(&copy), "Client demo copy");
    assert_eq!(
        name(&duplicate(dir.path(), &renamed).unwrap()),
        "Client demo copy 2"
    );
}

#[test]
fn update_keeps_parts_and_import_copies_in() {
    let dir = tempfile::tempdir().unwrap();
    let path = create(dir.path(), &project(), &["cursor"]).unwrap();
    let mut changed = project();
    changed.set("cursorSize", json!(5.));
    update(&path, &changed).unwrap();
    let data = load(&path).unwrap();
    assert_eq!(groups(&data), ["cursor"]);
    assert_eq!(data["snapshot"]["cursorSize"], json!(5.));

    let elsewhere = tempfile::tempdir().unwrap();
    let outside = elsewhere.path().join("Preset.json");
    std::fs::copy(&path, &outside).unwrap();
    let imported = import(dir.path(), &outside).unwrap();
    assert_eq!(name(&imported), "Preset 2");
    assert!(outside.exists());
    assert!(rename(dir.path(), &outside, "x").is_err(), "not managed");
}
