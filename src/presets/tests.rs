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
