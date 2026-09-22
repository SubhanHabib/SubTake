//! Caption line balancing, paging and animation ported from Recordly's
//! captionLayout.ts. Text measurement is supplied by the native font engine.
use crate::timeline::n;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Word {
    pub text: String,
    pub leading_space: bool,
    pub forced_break: bool,
    pub start: f64,
    pub end: f64,
    pub real: bool,
}

#[derive(Clone, Debug, Serialize)]
pub struct Line {
    pub words: Vec<Word>,
    pub width: f64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Layout {
    pub lines: Vec<Line>,
    pub opacity: f64,
    pub translate_y: f64,
    pub scale: f64,
    pub page: usize,
}

fn covered(cues: &[Value], time: f64) -> bool {
    let mut sorted: Vec<_> = cues.iter().collect();
    sorted.sort_by(|a, b| n(a, "startMs", 0.).total_cmp(&n(b, "startMs", 0.)));
    for (i, c) in sorted.iter().enumerate() {
        if time < n(c, "startMs", 0.) {
            return false;
        }
        let mut end = n(c, "endMs", 0.);
        if let Some(next) = sorted.get(i + 1)
            && n(next, "startMs", 0.) - end < 500.
        {
            end = end.max(n(next, "startMs", 0.));
        }
        if time <= end {
            return true;
        }
    }
    false
}

pub fn flatten(cues: &[Value]) -> Vec<Word> {
    let mut words = vec![];
    for (cue_index, cue) in cues.iter().enumerate() {
        let mut source: Vec<Word> = cue["words"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|w| {
                        let text = w["text"].as_str()?.trim().to_owned();
                        if text.is_empty() {
                            return None;
                        }
                        let start = n(w, "startMs", f64::NAN);
                        let end = n(w, "endMs", f64::NAN);
                        Some(Word {
                            text,
                            leading_space: w["leadingSpace"].as_bool().unwrap_or(false),
                            forced_break: false,
                            start,
                            end,
                            real: start.is_finite() && end.is_finite() && end > start,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();
        if source.is_empty() {
            for (line_index, line) in cue["text"]
                .as_str()
                .unwrap_or("")
                .lines()
                .filter(|l| !l.trim().is_empty())
                .enumerate()
            {
                for (index, text) in line.split_whitespace().enumerate() {
                    source.push(Word {
                        text: text.into(),
                        leading_space: true,
                        forced_break: line_index > 0 && index == 0,
                        start: 0.,
                        end: 0.,
                        real: false,
                    });
                }
            }
        }
        let real = source.iter().all(|w| w.real);
        let count = source.len();
        let start = n(cue, "startMs", 0.);
        let end = n(cue, "endMs", 0.);
        let step = (end - start).max(1.) / count.max(1) as f64;
        for (i, mut w) in source.into_iter().enumerate() {
            if !real {
                w.start = start + step * i as f64;
                w.end = (if i + 1 == count {
                    end
                } else {
                    start + step * (i + 1) as f64
                })
                .max(w.start + 1.);
                w.real = false;
            }
            if i == 0 {
                w.forced_break |= cue_index > 0;
                w.leading_space = false;
            }
            words.push(w);
        }
    }
    words
}

pub fn layout(
    cues: &[Value],
    time: f64,
    max_width: f64,
    max_rows: usize,
    animation: &str,
    measure: impl Fn(&str) -> f64,
) -> Option<Layout> {
    if !covered(cues, time) {
        return None;
    }
    let words = flatten(cues);
    let count = words.len();
    if count == 0 {
        return None;
    }
    let plain: Vec<_> = words.iter().map(|w| measure(&w.text)).collect();
    let segment: Vec<_> = words
        .iter()
        .map(|w| {
            measure(&format!(
                "{}{}",
                if w.leading_space { " " } else { "" },
                w.text
            ))
        })
        .collect();
    let mut cost = vec![f64::INFINITY; count + 1];
    let mut breaks = vec![count; count];
    cost[count] = 0.;
    for start in (0..count).rev() {
        let mut width = plain[start];
        for end in start..count {
            if end > start {
                if words[end].forced_break {
                    break;
                }
                width += segment[end];
            }
            if width > max_width && end > start {
                break;
            }
            let slack = (max_width - width).max(0.);
            let fullness = if max_width <= 0. {
                1.
            } else {
                width / max_width
            };
            let penalty = if end + 1 == count {
                slack * slack * 0.18
            } else {
                slack * slack
                    + if fullness < 0.72 {
                        (0.72 - fullness) * 26000.
                    } else {
                        0.
                    }
            };
            if penalty + cost[end + 1] < cost[start] {
                cost[start] = penalty + cost[end + 1];
                breaks[start] = end + 1;
            }
        }
    }
    let mut lines = vec![];
    let mut i = 0;
    while i < count {
        let end = breaks[i].max(i + 1);
        let mut line = words[i..end].to_vec();
        line[0].leading_space = false;
        let width = line
            .iter()
            .map(|w| {
                measure(&format!(
                    "{}{}",
                    if w.leading_space { " " } else { "" },
                    w.text
                ))
            })
            .sum();
        lines.push(Line { words: line, width });
        i = end;
    }
    let rows = max_rows.clamp(1, 4);
    let total = lines.len().div_ceil(rows);
    let mut pages: Vec<(Vec<Line>, f64, f64)> = vec![];
    let mut at = 0;
    for page in 0..total {
        let size = rows.min((lines.len() - at).div_ceil(total - page));
        let group = lines[at..at + size].to_vec();
        at += size;
        let start = group[0].words[0].start;
        let end = group.last()?.words.last()?.end;
        pages.push((group, start, end));
    }
    for i in 0..pages.len().saturating_sub(1) {
        pages[i].2 = (pages[i].1 + 1.).max(pages[i + 1].1);
    }
    if let Some(last) = pages.last_mut() {
        last.2 = (last.1 + 1.).max(words.last()?.end);
    }
    let (page, (lines, start, end)) = pages
        .into_iter()
        .enumerate()
        .find(|(_, (_, start, end))| time >= *start && time <= *end)?;
    let content_end = lines
        .iter()
        .flat_map(|l| l.words.iter())
        .map(|w| w.end)
        .fold(start, f64::max);
    let disappear = animation != "none" && !covered(cues, content_end + 1.);
    let exit = ((if disappear { content_end } else { end } - time) / 180.).clamp(0., 1.);
    let visibility = ((time - start) / 180.).clamp(0., 1.).min(exit);
    let (mut opacity, translate_y, scale) = match animation {
        "none" => (1., 0., 1.),
        "fade" => (0.3 + visibility * 0.7, 0., 1.),
        "rise" => (
            0.25 + visibility * 0.75,
            (1. - visibility) * 18.,
            0.985 + visibility * 0.015,
        ),
        _ => (
            0.35 + visibility * 0.65,
            (1. - visibility) * 8.,
            0.94 + visibility * 0.06,
        ),
    };
    if disappear {
        opacity *= exit;
    }
    Some(Layout {
        lines,
        opacity,
        translate_y,
        scale,
        page,
    })
}
