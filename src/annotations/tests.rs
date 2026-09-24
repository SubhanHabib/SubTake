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

#[test]
fn rows_follow_the_kind_and_read_its_values() {
    let blur = new("add-pixelate", &[], 16. / 9.).unwrap();
    let rows = rows(&blur);
    assert_eq!(rows[0].label, "Pixelate");
    let amount = rows
        .iter()
        .find(|r| r.key == "region.blurIntensity")
        .unwrap();
    assert_eq!(
        (amount.label, amount.value.clone()),
        ("Block size", json!(24.))
    );
    assert!(!rows.iter().any(|r| r.key == "region.style.fontFamily"));
    let text = rows_of("add-title");
    assert!(
        text.iter()
            .any(|r| r.key == "region.style.fontFamily" && r.value == "Helvetica")
    );
}

fn rows_of(action: &str) -> Vec<Row> {
    rows(&new(action, &[], 16. / 9.).unwrap())
}
