//! What each Add entry puts on the annotation lane: the kind's starting
//! settings. Positions and sizes are percent of the output; lengths in the
//! style are pixels at 1080p.

use crate::timeline::n;
use serde_json::{Value, json};

/// Every annotation the Add panel and menu offer, as (action, label, glyph).
pub const KINDS: [(&str, &str, &str); 11] = [
    ("add-title", "Title", "TextAa-regular"),
    ("add-lower-third", "Lower third", "TextAlignLeft-regular"),
    ("add-label", "Label", "Tag-regular"),
    ("add-figure", "Arrow", "ArrowUpRight-regular"),
    ("add-blur", "Blur", "Drop-regular"),
    ("add-pixelate", "Pixelate", "GridFour-regular"),
    ("add-highlight", "Highlight", "BoundingBox-regular"),
    ("add-spotlight", "Spotlight", "Flashlight-regular"),
    ("add-step", "Step", "NumberCircleOne-regular"),
    ("add-image", "Image", "Image-regular"),
    ("add-text", "Text", "TextT-regular"),
];

/// The colours an annotation's colour rows offer as swatches, before the
/// field that takes any other. `transparent` is "none".
pub const COLOURS: [&str; 9] = [
    "transparent",
    "#ffffff",
    "#111114",
    "#ff453a",
    "#ff9f0a",
    "#facc15",
    "#30d158",
    "#2563eb",
    "#bf5af2",
];

/// An annotation's kind, by its type, as its name and glyph: what its plate on
/// the lane shows.
pub fn kind_of(annotation: &Value) -> (&'static str, &'static str) {
    let action = match annotation["type"].as_str().unwrap_or("text") {
        "figure" => "add-figure",
        "blur" if annotation["blurMode"] == "pixelate" => "add-pixelate",
        "blur" => "add-blur",
        "highlight" => "add-highlight",
        "spotlight" => "add-spotlight",
        "step" => "add-step",
        "image" => "add-image",
        _ => "add-text",
    };
    KINDS
        .iter()
        .find(|(kind, _, _)| *kind == action)
        .map_or(("Text", "TextT-regular"), |(_, name, glyph)| {
            (*name, *glyph)
        })
}

/// The new annotation for `action`, over `existing` ones in an output
/// `aspect` (width over height) wide, or `None` for an action that adds no
/// annotation. An image's picture is left for the caller to choose.
pub fn new(action: &str, existing: &[Value], aspect: f64) -> Option<Value> {
    let text = |content: &str, style: Value| json!({"type":"text","content":content,"textContent":content,"style":style});
    let mut region = match action {
        "add-text" => text(
            "Your text",
            json!({"fontSize":64.,"fontFamily":"Helvetica","fontWeight":"bold","color":"#ffffff","backgroundColor":"transparent","textAlign":"center","borderRadius":8.}),
        ),
        "add-title" => text(
            "Title",
            json!({"fontSize":96.,"fontFamily":"Helvetica","fontWeight":"bold","color":"#ffffff","backgroundColor":"transparent","textAlign":"center","borderRadius":0.}),
        ),
        "add-lower-third" => text(
            "Name · Role",
            json!({"fontSize":44.,"fontFamily":"Helvetica","fontWeight":"bold","color":"#ffffff","backgroundColor":"#0f0f14c8","textAlign":"left","borderRadius":12.,"padding":28.}),
        ),
        "add-label" => text(
            "Label",
            json!({"fontSize":32.,"fontFamily":"Helvetica","fontWeight":"bold","color":"#ffffff","backgroundColor":"#2563eb","textAlign":"center","borderRadius":999.}),
        ),
        "add-figure" => {
            json!({"type":"figure","figureData":{"arrowDirection":"right","color":"#ff453a","strokeWidth":6.}})
        }
        "add-blur" => {
            json!({"type":"blur","blurMode":"blur","blurIntensity":20.,"style":{"borderRadius":12.}})
        }
        "add-pixelate" => {
            json!({"type":"blur","blurMode":"pixelate","blurIntensity":24.,"style":{"borderRadius":12.}})
        }
        "add-highlight" => {
            json!({"type":"highlight","figureData":{"color":"#facc15","strokeWidth":6.},"style":{"borderRadius":16.,"backgroundColor":"transparent"}})
        }
        "add-spotlight" => {
            json!({"type":"spotlight","dimOpacity":60.,"style":{"borderRadius":16.}})
        }
        "add-step" => {
            let number = existing.iter().filter(|a| a["type"] == "step").count() + 1;
            json!({"type":"step","textContent":number.to_string(),"figureData":{"color":"#2563eb"},"style":{"color":"#ffffff","fontFamily":"Helvetica"}})
        }
        "add-image" => json!({"type":"image"}),
        _ => return None,
    };
    let (x, y, width, height) = match action {
        "add-title" => (15., 38., 70., 18.),
        "add-lower-third" => (5., 76., 42., 11.),
        "add-label" => (42., 45., 16., 8.),
        "add-figure" => (38., 42., 20., 14.),
        "add-highlight" | "add-spotlight" => (35., 32., 30., 30.),
        // A square in the output, however wide it is.
        "add-step" => (47., 45., 6., 6. * aspect),
        "add-image" => (40., 35., 20., 20. * aspect),
        _ => (35., 40., 30., 20.),
    };
    region["position"] = json!({"x":x,"y":y});
    region["size"] = json!({"width":width,"height":height});
    // New annotations go on top, except a spotlight, which always draws
    // under the rest.
    let top = existing
        .iter()
        .map(|a| n(a, "zIndex", 0.))
        .fold(0., f64::max);
    region["zIndex"] = json!(if action == "add-spotlight" {
        0.
    } else {
        top + 1.
    });
    Some(region)
}

/// One inspector row for an annotation: its key under `region.`, label,
/// field kind (0 text, 1 slider, 5 section, 6 colour), slider range and the
/// annotation's value, or the kind's default where it has none.
pub struct Row {
    pub key: String,
    pub label: &'static str,
    pub kind: i32,
    pub range: (f32, f32),
    pub value: Value,
}

/// The inspector's rows for `annotation`, by its kind: what it is made of,
/// then where it sits.
pub fn rows(annotation: &Value) -> Vec<Row> {
    let (name, _) = kind_of(annotation);
    let mut rows = Vec::new();
    let mut add = |key: &str, label: &'static str, kind: i32, range: (f32, f32), default: Value| {
        let own = key
            .split('.')
            .try_fold(annotation, |value, part| value.get(part))
            .filter(|value| !value.is_null());
        rows.push(Row {
            key: if key.is_empty() {
                String::new()
            } else {
                format!("region.{key}")
            },
            label,
            kind,
            range,
            value: own.cloned().unwrap_or(default),
        });
    };
    let none = (0., 0.);
    let corners = (0., 120.);
    let thickness = (1., 24.);
    add("", name, 5, none, json!(""));
    match annotation["type"].as_str().unwrap_or("text") {
        "figure" => {
            add(
                "figureData.arrowDirection",
                "Direction",
                0,
                none,
                json!("right"),
            );
            add("figureData.color", "Colour", 6, none, json!("#ff453a"));
            add(
                "figureData.strokeWidth",
                "Thickness",
                1,
                thickness,
                json!(6),
            );
        }
        "blur" => {
            add("blurMode", "Effect", 0, none, json!("blur"));
            let amount = if name == "Pixelate" {
                "Block size"
            } else {
                "Strength"
            };
            add("blurIntensity", amount, 1, (2., 80.), json!(20));
            add("style.borderRadius", "Corners", 1, corners, json!(0));
        }
        "highlight" => {
            add("figureData.color", "Outline", 6, none, json!("#facc15"));
            add(
                "style.backgroundColor",
                "Fill",
                6,
                none,
                json!("transparent"),
            );
            add(
                "figureData.strokeWidth",
                "Thickness",
                1,
                thickness,
                json!(6),
            );
            add("style.borderRadius", "Corners", 1, corners, json!(16));
        }
        "spotlight" => {
            add("dimOpacity", "Dim outside", 1, (0., 100.), json!(60));
            add("style.borderRadius", "Corners", 1, corners, json!(16));
        }
        "step" => {
            add("textContent", "Number", 0, none, json!("1"));
            add("figureData.color", "Badge", 6, none, json!("#2563eb"));
            add("style.color", "Number colour", 6, none, json!("#ffffff"));
        }
        "image" => {}
        _ => {
            add("textContent", "Text", 0, none, json!(""));
            add("style.fontFamily", "Font", 0, none, json!("Helvetica"));
            add("style.fontWeight", "Weight", 0, none, json!("bold"));
            add("style.textAlign", "Align", 0, none, json!("center"));
            add("style.fontSize", "Size", 1, (8., 240.), json!(64));
            add("style.color", "Colour", 6, none, json!("#ffffff"));
            add(
                "style.backgroundColor",
                "Box",
                6,
                none,
                json!("transparent"),
            );
            add("style.borderRadius", "Box corners", 1, corners, json!(8));
            add("style.padding", "Box padding", 1, (0., 80.), json!(8));
        }
    }
    add("", "Placement", 5, none, json!(""));
    let percent = (0., 100.);
    add("position.x", "Left", 1, percent, json!(50));
    add("position.y", "Top", 1, percent, json!(50));
    add("size.width", "Width", 1, percent, json!(30));
    add("size.height", "Height", 1, percent, json!(20));
    rows
}

#[cfg(test)]
mod tests;
