//! Subtitle sidecars use the same output clock as the rendered video.
use crate::{
    project::Project,
    timeline::{Span, n},
};
use anyhow::Result;
use serde_json::{Value, json};
use std::{io::Write, path::Path};
pub fn mapped_cues(project: &Project, spans: &[Span]) -> Vec<Value> {
    let mut output = vec![];
    for span in spans {
        for cue in project.regions("autoCaptions") {
            let a = (n(cue, "startMs", 0.) / 1000.).max(span.source_start);
            let b = (n(cue, "endMs", 0.) / 1000.).min(span.source_end);
            let text = cue["text"].as_str().unwrap_or("").trim();
            if b > a && !text.is_empty() {
                output.push(json!({"startMs":((span.output_start+(a-span.source_start)/span.speed)*1000.).round(),"endMs":((span.output_start+(b-span.source_start)/span.speed)*1000.).round(),"text":text}));
            }
        }
    }
    output.sort_by(|a, b| n(a, "startMs", 0.).total_cmp(&n(b, "startMs", 0.)));
    output
}
fn timestamp(ms: f64, vtt: bool) -> String {
    let ms = ms.max(0.).round() as u64;
    format!(
        "{:02}:{:02}:{:02}{}{:03}",
        ms / 3600000,
        ms / 60000 % 60,
        ms / 1000 % 60,
        if vtt { '.' } else { ',' },
        ms % 1000
    )
}
pub fn format(cues: &[Value], vtt: bool) -> String {
    let mut text = if vtt {
        "WEBVTT\n\n".to_owned()
    } else {
        String::new()
    };
    for (index, cue) in cues.iter().enumerate() {
        if !vtt {
            text.push_str(&format!("{}\n", index + 1));
        }
        text.push_str(&format!(
            "{} --> {}\n{}\n\n",
            timestamp(n(cue, "startMs", 0.), vtt),
            timestamp(n(cue, "endMs", 0.), vtt),
            cue["text"].as_str().unwrap_or("")
        ));
    }
    text
}
pub fn write(project: &Project, spans: &[Span], video: &Path) -> Result<()> {
    let cues = mapped_cues(project, spans);
    for (extension, vtt) in [("srt", false), ("vtt", true)] {
        let destination = video.with_extension(extension);
        let mut file =
            tempfile::NamedTempFile::new_in(destination.parent().unwrap_or(Path::new(".")))?;
        file.write_all(format(&cues, vtt).as_bytes())?;
        file.as_file().sync_all()?;
        file.persist(destination).map_err(|e| e.error)?;
    }
    Ok(())
}
