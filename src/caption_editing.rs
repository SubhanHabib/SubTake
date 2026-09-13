//! Shared caption word normalization and adjacent split/merge commands.
use crate::{project::Project, segmentation::text_from_words, timeline::n};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
fn clean(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}
pub fn normalized_words(cue: &Value) -> Vec<Value> {
    let a = n(cue, "startMs", 0.);
    let b = n(cue, "endMs", a + 1.);
    let mut ws: Vec<Value> = cue["words"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|w| w["text"].as_str().is_some_and(|t| !clean(t).is_empty()))
                .cloned()
                .collect()
        })
        .unwrap_or_default();
    if ws.is_empty() {
        let tokens: Vec<_> = cue["text"]
            .as_str()
            .unwrap_or("")
            .split_whitespace()
            .collect();
        let length = tokens.len();
        let start = a.round().max(0.);
        let end = b.round().max(start + 1.);
        let duration = end - start;
        ws = tokens
            .into_iter()
            .enumerate()
            .map(|(i, t)| {
                let x = (start + duration * i as f64 / length as f64)
                    .round()
                    .max(start)
                    .min(end - 1.);
                let y = (start + duration * (i + 1) as f64 / length as f64)
                    .round()
                    .max(x + 1.)
                    .min(end);
                let mut w = json!({"text":t,"startMs":x,"endMs":y});
                if i > 0 {
                    w["leadingSpace"] = json!(true);
                }
                w
            })
            .collect();
    }
    ws.into_iter()
        .map(|w| {
            let x = n(&w, "startMs", a).round().min(b - 1.).max(a);
            let y = n(&w, "endMs", b).round().min(b).max(x + 1.);
            let mut result =
                json!({"text":clean(w["text"].as_str().unwrap()),"startMs":x,"endMs":y});
            if w["leadingSpace"] == true {
                result["leadingSpace"] = json!(true);
            }
            result
        })
        .collect()
}
fn spacing(mut ws: Vec<Value>) -> Vec<Value> {
    ws.sort_by(|a, b| {
        n(a, "startMs", 0.)
            .total_cmp(&n(b, "startMs", 0.))
            .then(n(a, "endMs", 0.).total_cmp(&n(b, "endMs", 0.)))
    });
    for (i, w) in ws.iter_mut().enumerate() {
        w.as_object_mut().unwrap().remove("leadingSpace");
        if i > 0 {
            w["leadingSpace"] = json!(true);
        }
    }
    ws
}
fn sorted(p: &Project) -> Vec<Value> {
    let mut cues = p.regions("autoCaptions").to_vec();
    cues.sort_by(|a, b| {
        n(a, "startMs", 0.)
            .total_cmp(&n(b, "startMs", 0.))
            .then(n(a, "endMs", 0.).total_cmp(&n(b, "endMs", 0.)))
    });
    cues
}
pub fn split(p: &mut Project, id: &str, time: f64) -> Result<String> {
    ensure!(time.is_finite(), "Invalid split time");
    let mut cues = sorted(p);
    let i = cues
        .iter()
        .position(|c| c["id"] == id)
        .context("Caption no longer exists")?;
    let cue = cues[i].clone();
    let words = normalized_words(&cue);
    ensure!(
        words.len() >= 2,
        "A caption needs at least two words to split"
    );
    let split = (1..words.len())
        .min_by(|a, b| {
            let distance = |i: usize| {
                ((n(&words[i - 1], "endMs", 0.) + n(&words[i], "startMs", 0.)) / 2. - time).abs()
            };
            distance(*a).total_cmp(&distance(*b))
        })
        .unwrap();
    let left = spacing(words[..split].to_vec());
    let right = spacing(words[split..].to_vec());
    let right_id = format!("caption-{}", uuid::Uuid::new_v4());
    let mut first = cue.clone();
    first["endMs"] = left.last().unwrap()["endMs"].clone();
    first["text"] = json!(text_from_words(&left));
    first["words"] = json!(left);
    let mut second = cue;
    second["id"] = json!(right_id);
    second["startMs"] = right[0]["startMs"].clone();
    second["text"] = json!(text_from_words(&right));
    second["words"] = json!(right);
    cues.splice(i..=i, [first, second]);
    p.set("autoCaptions", json!(cues));
    Ok(right_id)
}
pub fn merge_next(p: &mut Project, id: &str) -> Result<()> {
    let mut cues = sorted(p);
    let i = cues
        .iter()
        .position(|c| c["id"] == id)
        .context("Caption no longer exists")?;
    ensure!(i + 1 < cues.len(), "There is no following caption");
    let mut ws = normalized_words(&cues[i]);
    ws.extend(normalized_words(&cues[i + 1]));
    let ws = spacing(ws);
    let next = cues.remove(i + 1);
    cues[i]["endMs"] = next["endMs"].clone();
    cues[i]["text"] = json!(text_from_words(&ws));
    cues[i]["words"] = json!(ws);
    p.set("autoCaptions", json!(cues));
    Ok(())
}
pub fn edit_word(p: &mut Project, id: &str, index: usize, key: &str, value: &str) -> Result<()> {
    let cue = p
        .regions("autoCaptions")
        .iter()
        .find(|c| c["id"] == id)
        .context("Caption no longer exists")?;
    let mut ws = normalized_words(cue);
    let word = ws.get_mut(index).context("Word no longer exists")?;
    match key {
        "text" => {
            let t = clean(value);
            ensure!(!t.is_empty(), "Word text cannot be empty");
            word["text"] = json!(t);
        }
        "startMs" | "endMs" => {
            let time: f64 = value.parse()?;
            ensure!(time.is_finite(), "Word time must be finite");
            word[key] = json!(time.round());
        }
        _ => anyhow::bail!("Unknown word field"),
    }
    ensure!(
        n(word, "startMs", 0.) >= n(cue, "startMs", 0.)
            && n(word, "endMs", 0.) <= n(cue, "endMs", 0.)
            && n(word, "endMs", 0.) > n(word, "startMs", 0.),
        "Word times must fit inside the caption and end after they start"
    );
    let ws = spacing(ws);
    p.change_region(
        "autoCaptions",
        id,
        json!({"text":text_from_words(&ws),"words":ws}),
    )
}
