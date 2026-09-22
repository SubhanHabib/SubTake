//! Phrase segmentation ported from the current Electron caption pipeline.
//! Sentence boundaries use original words; acoustic silence trims fallback cues.
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Silence {
    pub start_ms: f64,
    pub end_ms: f64,
}
fn number(v: &Value, key: &str) -> f64 {
    v[key].as_f64().unwrap_or(0.)
}
fn start(v: &Value) -> f64 {
    number(v, "startMs")
}
fn end(v: &Value) -> f64 {
    number(v, "endMs")
}
fn text(v: &Value) -> &str {
    v["text"].as_str().unwrap_or("")
}
fn words(v: &Value) -> Vec<Value> {
    v["words"].as_array().cloned().unwrap_or_default()
}
fn order(a: &Value, b: &Value) -> std::cmp::Ordering {
    start(a)
        .total_cmp(&start(b))
        .then(end(a).total_cmp(&end(b)))
}
fn utf16_len(s: &str) -> usize {
    s.encode_utf16().count()
}

pub fn parse_silences(log: &str) -> Vec<Silence> {
    let mut intervals = vec![];
    let mut pending = None;
    for line in log.lines() {
        let value = |markers: &[&str]| {
            markers
                .iter()
                .find_map(|m| {
                    line.split_once(m)
                        .and_then(|(_, s)| s.split_whitespace().next()?.parse::<f64>().ok())
                })
                .filter(|v| v.is_finite())
        };
        if let Some(a) = value(&["silence_start:", "lavfi.silence_start="]) {
            pending = Some((a * 1000.).round().max(0.));
        } else if let Some(b) = value(&["silence_end:", "lavfi.silence_end="])
            && let Some(a) = pending.take()
        {
            let b = (b * 1000.).round();
            if b > a {
                intervals.push(Silence {
                    start_ms: a,
                    end_ms: b,
                });
            }
        }
    }
    if let Some(a) = pending {
        intervals.push(Silence {
            start_ms: a,
            end_ms: f64::INFINITY,
        });
    }
    intervals.sort_by(|a, b| a.start_ms.total_cmp(&b.start_ms));
    intervals
}
pub fn ends_sentence(s: &str) -> bool {
    let s = s
        .trim()
        .trim_end_matches(|c: char| ")] }\"'”’»」』）】］｝>".contains(c))
        .trim();
    if !s.ends_with(['.', '?', '!', '…', '。', '！', '？']) {
        return false;
    }
    if let Some(core) = s.strip_suffix('.') {
        let core = core.to_lowercase();
        if [
            "mr", "mrs", "ms", "dr", "prof", "sr", "jr", "st", "vs", "etc",
        ]
        .contains(&core.as_str())
        {
            return false;
        }
        if !core.is_empty()
            && core
                .split('.')
                .all(|p| p.len() == 1 && p.as_bytes()[0].is_ascii_lowercase())
        {
            return false;
        }
    }
    true
}
pub fn text_from_words(words: &[Value]) -> String {
    let mut result = String::new();
    for (i, w) in words.iter().enumerate() {
        if i > 0 && w["leadingSpace"] == true {
            result.push(' ');
        }
        result.push_str(text(w));
    }
    result.trim().into()
}
fn piece(mut words: Vec<Value>) -> Value {
    if let Some(w) = words.first_mut()
        && w["leadingSpace"] == true
    {
        w.as_object_mut().unwrap().remove("leadingSpace");
    }
    json!({"id":"","startMs":start(&words[0]),"endMs":end(words.last().unwrap()),"text":text_from_words(&words),"words":words})
}
fn pad(cues: &mut [Value]) {
    for i in 0..cues.len() {
        let (a, b) = (start(&cues[i]), end(&cues[i]));
        let left = if i == 0 {
            80_f64.min(a)
        } else {
            80_f64.min(((a - end(&cues[i - 1])).max(0.) / 2.).floor())
        };
        let right = if i + 1 == cues.len() {
            80.
        } else {
            80_f64.min(((start(&cues[i + 1]) - b).max(0.) / 2.).floor())
        };
        let a = (a - left).round().max(0.);
        cues[i]["startMs"] = json!(a);
        cues[i]["endMs"] = json!((b + right).round().max(a + 1.));
    }
}
fn merge_short(cues: Vec<Value>) -> Vec<Value> {
    let mut merged: Vec<Value> = vec![];
    for next in cues {
        if let Some(group) = merged.last_mut()
            && end(group) - start(group) < 800.
            && end(&next) - start(&next) < 800.
            && start(&next) - end(group) <= 400.
            && end(&next) - start(group) <= 2500.
            && utf16_len(text(group)) + utf16_len(text(&next)) < 80
        {
            let (mut left, mut right) = (words(group), words(&next));
            let mut joined = if !left.is_empty() && !right.is_empty() {
                right[0]["leadingSpace"] = json!(true);
                left.extend(right);
                piece(left)
            } else {
                json!({"id":"","text":format!("{} {}",text(group),text(&next)).trim()})
            };
            joined["startMs"] = json!(start(group));
            joined["endMs"] = json!(end(&next));
            *group = joined;
            continue;
        }
        merged.push(next);
    }
    merged
}
fn sentences(cue: Value) -> Vec<Value> {
    let ws = words(&cue);
    if !ws.is_empty() {
        let mut groups = vec![];
        let mut current = vec![];
        for w in ws {
            let stop = ends_sentence(text(&w));
            current.push(w);
            if stop {
                groups.push(std::mem::take(&mut current));
            }
        }
        if !current.is_empty() {
            groups.push(current);
        }
        if groups.len() <= 1 {
            return vec![cue];
        }
        return groups.into_iter().map(piece).collect();
    }
    let mut groups: Vec<String> = vec![];
    let mut current = String::new();
    for token in text(&cue).split_whitespace() {
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(token);
        if ends_sentence(token) {
            groups.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        groups.push(current);
    }
    if groups.len() <= 1 {
        return vec![cue];
    }
    let total = groups.iter().map(|g| utf16_len(g)).sum::<usize>().max(1) as f64;
    let duration = (end(&cue) - start(&cue)).max(1.);
    let count = groups.len();
    let mut cursor = start(&cue);
    groups
        .into_iter()
        .enumerate()
        .map(|(i, t)| {
            let a = cursor;
            let b = if i + 1 == count {
                end(&cue)
            } else {
                (a + duration * utf16_len(&t) as f64 / total)
                    .round()
                    .min(end(&cue) - 1.)
            };
            cursor = b.max(a + 1.);
            json!({"id":"","text":t,"startMs":a,"endMs":cursor})
        })
        .collect()
}
fn silence_fallback(cues: &[Value], silences: &[Silence]) -> Vec<Value> {
    let a = start(&cues[0]);
    let b = cues.iter().map(end).fold(a, f64::max);
    let mut boundaries: Vec<_> = silences
        .iter()
        .map(|s| (s.start_ms.max(a), s.end_ms.min(b)))
        .filter(|(x, y)| y > x && (y - x >= 1500. || *x <= a || *y >= b))
        .collect();
    boundaries.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut regions = vec![];
    let mut cursor = a;
    for (x, y) in boundaries {
        if x > cursor {
            regions.push((cursor, x));
        }
        cursor = cursor.max(y);
    }
    if cursor < b {
        regions.push((cursor, b));
    }
    regions.retain(|(a, b)| b - a >= 150.);
    let mut texts = vec![String::new(); regions.len()];
    let mut region_words = vec![vec![]; regions.len()];
    for cue in cues {
        let overlaps: Vec<_> = regions
            .iter()
            .enumerate()
            .filter(|(_, r)| start(cue) < r.1 && end(cue) > r.0)
            .collect();
        if overlaps.is_empty() {
            continue;
        }
        let ws = words(cue);
        if !ws.is_empty() {
            for w in ws {
                let center = (start(&w) + end(&w)) / 2.;
                let (i, _) = overlaps
                    .iter()
                    .min_by(|(_, a), (_, b)| {
                        let distance = |r: &&(f64, f64)| {
                            if center < r.0 {
                                r.0 - center
                            } else if center > r.1 {
                                center - r.1
                            } else {
                                0.
                            }
                        };
                        distance(a).total_cmp(&distance(b))
                    })
                    .unwrap();
                region_words[*i].push(w);
            }
            continue;
        }
        let tokens: Vec<_> = text(cue).split_whitespace().collect();
        let total = overlaps
            .iter()
            .map(|(_, r)| end(cue).min(r.1) - start(cue).max(r.0))
            .sum::<f64>()
            .max(1.);
        let mut token = 0;
        for (pos, (i, r)) in overlaps.iter().enumerate() {
            let count = if pos + 1 == overlaps.len() {
                tokens.len() - token
            } else {
                ((tokens.len() as f64 * (end(cue).min(r.1) - start(cue).max(r.0)) / total).round()
                    as usize)
                    .min(tokens.len() - token)
            };
            let addition = tokens[token..token + count].join(" ");
            token += count;
            if !addition.is_empty() {
                if !texts[*i].is_empty() {
                    texts[*i].push(' ');
                }
                texts[*i].push_str(&addition);
            }
        }
    }
    let mut pieces:Vec<Value>=regions.into_iter().enumerate().map(|(i,(a,b))|{if region_words[i].is_empty(){json!({"id":"","startMs":a,"endMs":b,"text":texts[i]})}else{region_words[i].sort_by(order);let ws=std::mem::take(&mut region_words[i]);json!({"id":"","startMs":start(&ws[0]),"endMs":end(ws.last().unwrap()),"text":text_from_words(&ws),"words":ws})}}).filter(|v|!text(v).trim().is_empty()).collect();
    pieces.sort_by(order);
    pad(&mut pieces);
    pieces.into_iter().flat_map(sentences).collect()
}
pub fn segment(cues: &[Value], silences: &[Silence]) -> Vec<Value> {
    if cues.is_empty() {
        return vec![];
    }
    let mut sorted = cues.to_vec();
    sorted.sort_by(order);
    let mut result = if cues.iter().all(|c| !words(c).is_empty()) {
        let mut stream: Vec<Value> = vec![];
        for cue in &sorted {
            for (i, w) in words(cue).into_iter().enumerate() {
                let leading = !stream.is_empty() && (i == 0 || w["leadingSpace"] != false);
                let mut word = json!({"text":text(&w),"startMs":start(&w),"endMs":end(&w)});
                if leading {
                    word["leadingSpace"] = json!(true);
                }
                stream.push(word);
            }
        }
        stream.sort_by(order);
        stream.retain(|w| {
            !silences.iter().any(|s| {
                s.end_ms - s.start_ms >= 1500. && s.start_ms <= start(w) && end(w) <= s.end_ms
            })
        });
        let mut pieces = vec![];
        let mut current = vec![];
        for (i, w) in stream.iter().enumerate() {
            current.push(w.clone());
            let stop = stream.get(i + 1).is_some_and(|next| {
                let gap = start(next) - end(w);
                ends_sentence(text(w))
                    || gap >= 700.
                    || (gap > 0.
                        && silences.iter().any(|s| {
                            s.end_ms - s.start_ms >= 1500.
                                && s.start_ms < start(next)
                                && s.end_ms > end(w)
                        }))
                    || end(w) - start(&current[0]) >= 12000.
            });
            if stop {
                pieces.push(piece(std::mem::take(&mut current)));
            }
        }
        if !current.is_empty() {
            pieces.push(piece(current));
        }
        pieces.retain(|c| !text(c).trim().is_empty());
        pieces.sort_by(order);
        let mut merged = merge_short(pieces);
        pad(&mut merged);
        merged
    } else {
        merge_short(silence_fallback(&sorted, silences))
    };
    for (i, c) in result.iter_mut().enumerate() {
        c["id"] = json!(format!("caption-{}", i + 1));
    }
    result
}
