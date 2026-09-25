use super::*;
use base64::Engine;

#[test]
fn source_still_decodes_the_platforms_jpeg() {
    let mut jpeg = Vec::new();
    image::RgbImage::from_pixel(8, 4, image::Rgb([200, 40, 40]))
        .write_to(
            &mut std::io::Cursor::new(&mut jpeg),
            image::ImageFormat::Jpeg,
        )
        .unwrap();
    let encoded = base64::engine::general_purpose::STANDARD.encode(&jpeg);
    let still = source_still(&serde_json::json!({ "thumbnail": encoded }));
    let still = still.0.expect("a still");
    let size = still.size(0);
    assert_eq!((size.width.0, size.height.0), (8, 4));
}

#[test]
fn source_still_falls_back_to_the_placeholder() {
    assert!(source_still(&serde_json::json!({})).0.is_none());
    assert!(
        source_still(&serde_json::json!({ "thumbnail": "not base64" }))
            .0
            .is_none()
    );
}

#[test]
fn recorder_webcam_takes_the_camera_cards_choices() {
    let mut preferences = subtake_native::preferences::Preferences::default();
    let mut settings = serde_json::json!({ "mirror": true, "margin": 24 });
    recorder_webcam(&mut settings, &preferences, (1920, 1080));
    // M is 22% of the width: 422 of 1920, which is 39.1% of the 1080 side.
    assert_eq!(settings["width"], 39.1);
    assert_eq!(settings["height"], 39.1);
    assert_eq!(
        (
            settings["positionX"].as_f64(),
            settings["positionY"].as_f64()
        ),
        (Some(1.), Some(1.))
    );
    assert_eq!(settings["roundness"], 100.);
    assert_eq!(settings["margin"], 24, "what the card does not set is kept");

    for (key, value) in [
        ("camera-corner", "top-left"),
        ("camera-shape", "square"),
        ("camera-size", "s"),
    ] {
        preferences.recorder.insert(key.into(), value.into());
    }
    recorder_webcam(&mut settings, &preferences, (1080, 1920));
    assert_eq!(settings["width"], 16.);
    assert_eq!(
        (
            settings["positionX"].as_f64(),
            settings["positionY"].as_f64()
        ),
        (Some(0.), Some(0.))
    );
    assert_eq!(settings["roundness"], 0.);
}

#[test]
fn input_gain_follows_the_level_squared() {
    assert_eq!(input_gain("100"), 1.);
    assert_eq!(input_gain("50"), 0.25);
    assert_eq!(input_gain("0"), 0.);
    // Past 100 the level holds there; unreadable, it is full.
    assert_eq!(input_gain("140"), 1.);
    assert_eq!(input_gain(""), 1.);
}

fn displays() -> Vec<Value> {
    vec![
        json!({
            "kind": "display", "nativeId": 1, "name": "Display 1 · 1512 × 982",
            "x": 0., "y": 0., "width": 1512., "height": 982., "thumbnail": "still",
        }),
        json!({
            "kind": "display", "nativeId": 3, "name": "Display 2 · 2560 × 1440",
            "x": 1512., "y": -200., "width": 2560., "height": 1440.,
        }),
        json!({ "kind": "window", "nativeId": 1, "name": "Safari — Docs" }),
    ]
}

#[test]
fn area_source_is_the_area_on_its_display() {
    let area = area_source(&displays(), "3 100 50 1280 720").expect("an area");
    assert_eq!(area["kind"], "area");
    assert_eq!(area["nativeId"], 3);
    assert_eq!(area["name"], "Area · 1280 × 720");
    assert_eq!(area["display"], "Display 2");
    assert_eq!(
        [
            &area["areaX"],
            &area["areaY"],
            &area["areaWidth"],
            &area["areaHeight"]
        ],
        [&json!(100.), &json!(50.), &json!(1280.), &json!(720.)]
    );
    // On the desktop, as a window's frame is.
    assert_eq!(
        [&area["x"], &area["y"], &area["width"], &area["height"]],
        [&json!(1612.), &json!(-150.), &json!(1280.), &json!(720.)]
    );
    let source = capture_source(&area);
    assert_eq!(
        (source.name.as_str(), source.detail.as_str()),
        ("Area", "1280 × 720")
    );
    let drawn = source.area.expect("where it sits");
    assert_eq!(drawn.display, "Display 2");
    assert!((drawn.aspect - 16. / 9.).abs() < 1e-6);
    assert_eq!(drawn.share, [100. / 2560., 50. / 1440., 0.5, 0.5]);
}

#[test]
fn area_source_keeps_the_area_on_its_display() {
    let area = area_source(&displays(), "1 1400 900 400 400").expect("an area");
    assert_eq!(area["name"], "Area · 112 × 82");
    assert_eq!(area["thumbnail"], "still");
}

#[test]
fn area_source_needs_its_display_and_a_whole_setting() {
    for setting in [
        "",
        "3",
        "3 1 2 3",
        "3 a 2 3 4",
        "3 0 0 1 1",
        "9 0 0 100 100",
        "3 0 0 100 100 5",
    ] {
        assert!(area_source(&displays(), setting).is_none(), "{setting:?}");
    }
    // A window with the display's id is not its display.
    assert!(area_source(&displays()[2..], "1 0 0 100 100").is_none());
    assert!(capture_source(&displays()[0]).area.is_none());
}

#[test]
fn area_aspect_reads_the_cards_locks() {
    assert_eq!(area_aspect("free"), None);
    assert_eq!(area_aspect(""), None);
    assert_eq!(area_aspect("16:9"), Some(16. / 9.));
    assert_eq!(area_aspect("9:16"), Some(9. / 16.));
    assert_eq!(area_aspect("1:0"), None);
}
