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
