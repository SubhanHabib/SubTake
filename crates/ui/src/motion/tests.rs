use super::*;

#[test]
fn a_state_tween_adopts_its_first_value_without_animating() {
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    // A panel opening with a switch already on must not play the
    // switch-on animation, so the first read arrives at the end state.
    assert_eq!(fades.set_state_at("s", true, false, t0), 1.0);
}

#[test]
fn a_state_tween_keeps_running_when_render_re_reads_it() {
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    fades.set_state_at("s", false, false, t0);
    // A state flip happens BETWEEN renders, so the frame counter moves
    // with it; two anchors inside one frame mean two controls fighting
    // over one key, which `set_state_at` deliberately snaps.
    fades.tick_at(t0);
    fades.set_state_at("s", true, false, t0);
    let half = t0 + Duration::from_millis(HOVER_FADE_MS / 2);
    let mid = fades.set_state_at("s", true, false, half);
    assert!(mid > 0.0 && mid < 1.0, "mid-flight value was {mid}");
    // Re-reading with an unchanged target must not re-anchor the tween:
    // render runs every frame, so that would freeze it at its origin.
    assert_eq!(
        fades.set_state_at("s", true, false, t0 + Duration::from_millis(HOVER_FADE_MS)),
        1.0
    );
}

#[test]
fn reversing_a_state_tween_mid_flight_stays_continuous() {
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    fades.set_state_at("s", false, false, t0);
    fades.tick_at(t0);
    fades.set_state_at("s", true, false, t0);
    let half = t0 + Duration::from_millis(HOVER_FADE_MS / 2);
    let mid = fades.set_state_at("s", true, false, half);
    fades.tick_at(half);
    assert_eq!(fades.set_state_at("s", false, false, half), mid);
}

#[test]
fn two_controls_sharing_a_key_never_settle() {
    // The failure mode this guards: if two controls end up with the same
    // tween key and disagree about their state, each render re-anchors
    // the other's tween, `tick_at` never reports settled, and the root
    // render requests a frame forever — the window repaints at full rate
    // while nothing on screen is moving.
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    let long_after = t0 + Duration::from_secs(10);
    fades.set_state_at("shared", true, false, long_after);
    fades.set_state_at("shared", false, false, long_after);
    assert!(
        !fades.tick_at(long_after),
        "a key whose target flips every render pegs the frame loop"
    );
}

#[test]
fn fade_runs_from_rest_to_hover_over_the_duration() {
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    fades.set_at("a", true, false, t0);
    assert_eq!(fades.value_at("a", t0), 0.0);
    assert_eq!(
        fades.value_at("a", t0 + Duration::from_millis(HOVER_FADE_MS)),
        1.0
    );
}

#[test]
fn reduced_motion_snaps() {
    let mut fades = HoverFades::default();
    let t0 = Instant::now();
    fades.set_at("a", true, true, t0);
    assert_eq!(fades.value_at("a", t0), 1.0);
}

#[test]
fn a_leave_on_a_never_hovered_key_creates_nothing() {
    let mut fades = HoverFades::default();
    fades.set_at("ghost", false, false, Instant::now());
    assert!(fades.entries.is_empty());
}

#[test]
fn tailwind_curve_is_monotonic_between_the_endpoints() {
    assert_eq!(EASE.eval(0.0), 0.0);
    assert!((EASE.eval(1.0) - 1.0).abs() < 1e-3);
    let mut previous = 0.0;
    for step in 0..=20 {
        let value = EASE.eval(step as f32 / 20.0);
        assert!(value >= previous - 1e-4, "curve must not go backwards");
        previous = value;
    }
}

#[test]
fn an_eased_move_starts_where_it_left_and_lands_exactly_on_its_target() {
    let t0 = Instant::now();
    let at = |ms| t0 + Duration::from_millis(ms);
    assert_eq!(ease_toward(40., 200., t0, 100, t0), 40.);
    let mid = ease_toward(40., 200., t0, 100, at(50));
    // Ease-out: more than half the travel is done by half the time.
    assert!(mid > 120. && mid < 200.);
    assert_eq!(ease_toward(40., 200., t0, 100, at(100)), 200.);
    assert_eq!(ease_toward(40., 200., t0, 100, at(500)), 200.);
    assert_eq!(progress(t0, 100, at(25)), 0.25);
    assert_eq!(progress(t0, 100, at(900)), 1.);
}
