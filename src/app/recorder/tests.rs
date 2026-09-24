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
