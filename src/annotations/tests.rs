use super::*;

#[test]
fn every_kind_adds_an_annotation() {
    for (action, _, _) in KINDS {
        assert!(new(action, &[], 16. / 9.).is_some(), "{action}");
    }
    assert!(new("add-zoom", &[], 16. / 9.).is_none());
}

#[test]
fn steps_count_up_and_stack_on_top() {
    let first = new("add-step", &[], 16. / 9.).unwrap();
    let second = new("add-step", std::slice::from_ref(&first), 16. / 9.).unwrap();
    assert_eq!(first["textContent"], "1");
    assert_eq!(second["textContent"], "2");
    assert_eq!(second["zIndex"], 2.);
    let spotlight = new("add-spotlight", &[first, second], 16. / 9.).unwrap();
    assert_eq!(spotlight["zIndex"], 0.);
}
