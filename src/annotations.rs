//! What each Add entry puts on the annotation lane: the kind's starting
//! settings. Positions and sizes are percent of the output; lengths in the
//! style are pixels at 1080p.

use crate::timeline::n;
use serde_json::{Value, json};

/// Every annotation the Add panel and menu offer, as (action, label, glyph).
pub const KINDS: [(&str, &str, &str); 11] = [
    ("add-title", "Title", "TextT-regular"),
    ("add-lower-third", "Lower third", "TextAlignLeft-regular"),
    ("add-label", "Label", "Tag-regular"),
    ("add-figure", "Arrow", "ArrowUpRight-regular"),
    ("add-blur", "Blur", "Drop-regular"),
    ("add-pixelate", "Pixelate", "GridFour-regular"),
    ("add-highlight", "Highlight", "BoundingBox-regular"),
    ("add-spotlight", "Spotlight", "Flashlight-regular"),
    ("add-step", "Step", "NumberCircleOne-regular"),
    ("add-image", "Image", "Image-regular"),
    ("add-text", "Text", "TextAa-regular"),
];

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
            json!({"fontSize":44.,"fontFamily":"Helvetica","fontWeight":"bold","color":"#ffffff","backgroundColor":"#0f0f14c8","textAlign":"left","borderRadius":12.}),
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

#[cfg(test)]
mod tests;
