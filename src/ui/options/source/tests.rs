use super::on_preview;

fn close(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-5)
}

#[test]
fn an_area_on_a_display_of_the_previews_shape_is_where_it_is() {
    let share = [0.25, 0.1, 0.5, 0.4];
    assert!(close(on_preview(share, 1.6, 1.6), share));
}

#[test]
fn a_wider_display_is_cropped_at_its_sides() {
    // 32:10 in a 16:10 box shows the middle half across.
    assert!(close(
        on_preview([0.25, 0., 0.5, 1.], 3.2, 1.6),
        [0., 0., 1., 1.]
    ));
    assert!(close(
        on_preview([0.375, 0.2, 0.25, 0.5], 3.2, 1.6),
        [0.25, 0.2, 0.5, 0.5]
    ));
}

#[test]
fn a_taller_display_is_cropped_at_its_top_and_foot() {
    // 8:10 in a 16:10 box shows the middle half down.
    assert!(close(
        on_preview([0.1, 0.375, 0.8, 0.25], 0.8, 1.6),
        [0.1, 0.25, 0.8, 0.5]
    ));
}

#[test]
fn an_area_running_into_the_crop_is_cut_there() {
    assert!(close(
        on_preview([0., 0., 0.5, 1.], 3.2, 1.6),
        [0., 0., 0.5, 1.]
    ));
    assert!(close(
        on_preview([0., 0., 0.2, 1.], 3.2, 1.6),
        [0., 0., 0., 1.]
    ));
}
