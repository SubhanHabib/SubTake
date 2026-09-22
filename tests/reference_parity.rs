use serde_json::{Value, json};
use std::path::Path;
use subtake_native::{
    geometry,
    motion::{Spring, cursor_config},
    project::Project,
    timeline,
};

fn corpus() -> Value {
    serde_json::from_str(include_str!("reference-fixtures.json")).unwrap()
}

fn near(a: f64, b: f64) {
    assert!((a - b).abs() < 1e-8, "{a} differs from reference {b}");
}

#[test]
fn frame_geometry_matches_production_typescript() {
    for case in corpus()["geometries"].as_array().unwrap() {
        let i = &case["input"];
        let r = &case["result"];
        let mut p = Project::new(Path::new("fixture.mp4"));
        p.set("padding", i["padding"].clone());
        p.set("cropRegion", i["cropRegion"].clone());
        let frame = geometry::frame(
            &p,
            i["width"].as_f64().unwrap(),
            i["height"].as_f64().unwrap(),
            i["videoWidth"].as_f64().unwrap(),
            i["videoHeight"].as_f64().unwrap(),
        );
        near(frame.x, r["centerOffsetX"].as_f64().unwrap());
        near(frame.y, r["centerOffsetY"].as_f64().unwrap());
        near(frame.width, r["croppedDisplayWidth"].as_f64().unwrap());
        near(frame.height, r["croppedDisplayHeight"].as_f64().unwrap());
    }
}

#[test]
fn cursor_springs_match_production_typescript() {
    for case in corpus()["springs"].as_array().unwrap() {
        let mut p = Project::new(Path::new("fixture.mp4"));
        p.set("cursorSmoothing", case["smoothing"].clone());
        let (k, c, m) = cursor_config(&p);
        near(k, case["config"]["stiffness"].as_f64().unwrap());
        near(c, case["config"]["damping"].as_f64().unwrap());
        near(m, case["config"]["mass"].as_f64().unwrap());
        let mut spring = Spring::default();
        for (i, expected) in case["values"].as_array().unwrap().iter().enumerate() {
            let target = if i < 10 {
                0.2
            } else if i < 40 {
                0.8
            } else {
                0.35
            };
            near(
                spring.step(target, 1000. / 60., k, c, m),
                expected.as_f64().unwrap(),
            );
        }
    }
}

#[test]
fn zoom_strength_matches_production_typescript() {
    for case in corpus()["zooms"].as_array().unwrap() {
        near(
            timeline::region_strength(
                &case["region"],
                case["time"].as_f64().unwrap(),
                1522.575,
                1015.05,
            ),
            case["result"].as_f64().unwrap(),
        );
    }
}

#[test]
fn zero_zoom_is_identity() {
    let p = Project::new(Path::new("fixture.mp4"));
    let camera = timeline::camera(&p, 1500.);
    near(camera.scale, 1.);
    near(camera.progress, 0.);
}

#[test]
fn connected_zoom_holds_through_short_gap() {
    let mut p = Project::new(Path::new("fixture.mp4"));
    p.set("connectZooms", json!(true));
    p.set("zoomRegions",json!([{"id":"a","startMs":0,"endMs":3000,"depth":2,"focus":{"cx":0.3,"cy":0.5}},{"id":"b","startMs":4000,"endMs":7000,"depth":2,"focus":{"cx":0.7,"cy":0.5}}]));
    assert!(timeline::camera(&p, 3500.).scale > 1.49);
}

#[test]
fn caption_pages_and_animations_match_production() {
    for case in corpus()["captions"].as_array().unwrap() {
        let actual = subtake_native::captions::layout(
            case["cues"].as_array().unwrap(),
            case["timeMs"].as_f64().unwrap(),
            180.,
            case["settings"]["maxRows"].as_u64().unwrap() as usize,
            case["settings"]["animationStyle"].as_str().unwrap(),
            |s| s.encode_utf16().count() as f64 * 9.,
        );
        let expected = &case["result"];
        if expected.is_null() {
            assert!(actual.is_none(), "{case}");
            continue;
        }
        let actual = actual.unwrap_or_else(|| panic!("Missing caption: {case}"));
        assert_eq!(actual.page, expected["page"].as_u64().unwrap() as usize);
        near(actual.opacity, expected["opacity"].as_f64().unwrap());
        near(
            actual.translate_y,
            expected["translate_y"].as_f64().unwrap(),
        );
        near(actual.scale, expected["scale"].as_f64().unwrap());
        assert_eq!(
            actual.lines.len(),
            expected["lines"].as_array().unwrap().len()
        );
        for (line, reference) in actual
            .lines
            .iter()
            .zip(expected["lines"].as_array().unwrap())
        {
            near(line.width, reference["width"].as_f64().unwrap());
            let text = line
                .words
                .iter()
                .map(|w| format!("{}{}", if w.leading_space { " " } else { "" }, w.text))
                .collect::<String>();
            assert_eq!(text, reference["text"].as_str().unwrap());
        }
    }
}

#[test]
fn legacy_export_dimensions_match_production() {
    for case in corpus()["dimensions"].as_array().unwrap() {
        let mut p = Project::new(Path::new("fixture.mp4"));
        for (key, value) in [
            ("aspectRatio", "aspectRatio"),
            ("cropRegion", "cropRegion"),
            ("exportQuality", "quality"),
        ] {
            p.set(key, case[value].clone());
        }
        let actual = subtake_native::export::legacy_dimensions(
            &p,
            case["width"].as_f64().unwrap(),
            case["height"].as_f64().unwrap(),
            false,
        );
        assert_eq!(
            actual,
            (
                case["result"]["width"].as_u64().unwrap() as u32,
                case["result"]["height"].as_u64().unwrap() as u32
            ),
            "{case}"
        );
    }
}

#[test]
fn whisper_word_reconstruction_matches_production() {
    for case in corpus()["transcriptions"].as_array().unwrap() {
        let actual = subtake_native::transcription::parse_json(&case["input"].to_string()).unwrap();
        // JSON integer vs floating representation is not semantically significant.
        let actual: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&actual).unwrap()).unwrap();
        fn canonical(v: &Value) -> Value {
            match v {
                Value::Number(n) => json!(n.as_f64().unwrap()),
                Value::Array(a) => json!(a.iter().map(canonical).collect::<Vec<_>>()),
                Value::Object(o) => {
                    Value::Object(o.iter().map(|(k, v)| (k.clone(), canonical(v))).collect())
                }
                _ => v.clone(),
            }
        }
        assert_eq!(canonical(&actual), canonical(&case["result"]));
    }
}

#[test]
fn autozoom_clusters_match_production() {
    for case in corpus()["autozooms"].as_array().unwrap() {
        let input = &case["input"];
        let reserved = input["reservedSpans"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| (r["start"].as_f64().unwrap(), r["end"].as_f64().unwrap()))
            .collect::<Vec<_>>();
        let actual = subtake_native::autozoom::suggest(
            input["cursorTelemetry"].as_array().unwrap(),
            input["totalMs"].as_f64().unwrap(),
            &reserved,
        );
        let expected = case["result"].as_array().unwrap();
        assert_eq!(actual.len(), expected.len());
        for (a, b) in actual.iter().zip(expected) {
            near(a["startMs"].as_f64().unwrap(), b["start"].as_f64().unwrap());
            near(a["endMs"].as_f64().unwrap(), b["end"].as_f64().unwrap());
            for axis in ["cx", "cy"] {
                near(
                    a["focus"][axis].as_f64().unwrap(),
                    b["focus"][axis].as_f64().unwrap(),
                );
            }
        }
    }
}

#[test]
fn phrase_segmentation_matches_production() {
    fn canonical(v: &Value) -> Value {
        match v {
            Value::Number(n) => json!(n.as_f64().unwrap()),
            Value::Array(a) => json!(a.iter().map(canonical).collect::<Vec<_>>()),
            Value::Object(o) => {
                Value::Object(o.iter().map(|(k, v)| (k.clone(), canonical(v))).collect())
            }
            _ => v.clone(),
        }
    }
    for (index, case) in corpus()["segments"].as_array().unwrap().iter().enumerate() {
        let silences: Vec<subtake_native::segmentation::Silence> =
            serde_json::from_value(case["silences"].clone()).unwrap();
        let result =
            subtake_native::segmentation::segment(case["cues"].as_array().unwrap(), &silences);
        assert_eq!(
            canonical(&json!(result)),
            canonical(&case["result"]),
            "case {index}: {case}"
        );
    }
    for case in corpus()["sentenceEnds"].as_array().unwrap() {
        assert_eq!(
            subtake_native::segmentation::ends_sentence(case["text"].as_str().unwrap()),
            case["result"].as_bool().unwrap()
        );
    }
}

#[test]
fn caption_split_and_merge_match_production() {
    fn canonical(v: &Value) -> Value {
        match v {
            Value::Number(n) => json!(n.as_f64().unwrap()),
            Value::Array(a) => json!(a.iter().map(canonical).collect::<Vec<_>>()),
            Value::Object(o) => {
                Value::Object(o.iter().map(|(k, v)| (k.clone(), canonical(v))).collect())
            }
            _ => v.clone(),
        }
    }
    for case in corpus()["captionEdits"].as_array().unwrap() {
        for operation in ["split", "merge"] {
            let mut p = Project::new(Path::new("fixture.mp4"));
            p.set("autoCaptions", case["cues"].clone());
            if operation == "split" {
                subtake_native::caption_editing::split(&mut p, "a", case["time"].as_f64().unwrap())
                    .unwrap();
            } else {
                subtake_native::caption_editing::merge_next(&mut p, "a").unwrap();
            }
            let actual = p
                .regions("autoCaptions")
                .iter()
                .cloned()
                .enumerate()
                .map(|(i, mut c)| {
                    c["id"] = json!(format!("canonical-{i}"));
                    c
                })
                .collect::<Vec<_>>();
            assert_eq!(
                canonical(&json!(actual)),
                canonical(&case[operation]),
                "{case}"
            );
        }
    }
}
