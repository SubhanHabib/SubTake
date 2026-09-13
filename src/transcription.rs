//! Local transcription is independent of the window system and recording backend.
use crate::{
    media::{self, ManagedChild},
    project::parse_srt,
};
use anyhow::{Context, Result, ensure};
use serde_json::{Value, json};
use std::{
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

/// Match the reference's whitespace-aware Whisper token reconstruction.
pub fn parse_json(content: &str) -> Result<Vec<Value>> {
    let value: Value = serde_json::from_str(content)?;
    let segments = value["transcription"]
        .as_array()
        .context("Whisper JSON has no transcription")?;
    let mut cues = vec![];
    for (index, segment) in segments.iter().enumerate() {
        let (Some(start), Some(end)) = (
            segment["offsets"]["from"].as_f64(),
            segment["offsets"]["to"].as_f64(),
        ) else {
            continue;
        };
        if start < 0. || end <= start {
            continue;
        }
        let mut words: Vec<Value> = vec![];
        let mut leading = false;
        let mut valid_words = true;
        if let Some(tokens) = segment["tokens"].as_array() {
            'tokens: for token in tokens {
                let text = token["text"].as_str().unwrap_or("");
                let (a, b) = (
                    token["offsets"]["from"].as_f64(),
                    token["offsets"]["to"].as_f64(),
                );
                let mut part = String::new();
                let flush = |part: &mut String, words: &mut Vec<Value>, leading: &mut bool| {
                    if part.is_empty() {
                        return;
                    }
                    if words.is_empty() || *leading {
                        let mut word = json!({"text":part,"startMs":a.unwrap().round(),"endMs":b.unwrap().round()});
                        if !words.is_empty() && *leading {
                            word["leadingSpace"] = json!(true);
                        }
                        words.push(word);
                    } else {
                        let word = words.last_mut().unwrap();
                        word["text"] = json!(format!("{}{}", word["text"].as_str().unwrap(), part));
                        word["endMs"] =
                            json!(word["endMs"].as_f64().unwrap().max(b.unwrap().round()));
                    }
                    part.clear();
                    *leading = false;
                };
                for ch in text.chars() {
                    if ch.is_whitespace() {
                        flush(&mut part, &mut words, &mut leading);
                        leading = !words.is_empty();
                    } else {
                        if !matches!((a,b),(Some(a),Some(b)) if a.is_finite() && b.is_finite() && b>a)
                        {
                            valid_words = false;
                            break 'tokens;
                        }
                        part.push(ch);
                    }
                }
                flush(&mut part, &mut words, &mut leading);
            }
        }
        if !valid_words {
            words.clear();
        }
        let text = if words.is_empty() {
            segment["text"].as_str().unwrap_or("").trim().to_owned()
        } else {
            words
                .iter()
                .enumerate()
                .map(|(i, w)| {
                    format!(
                        "{}{}",
                        if i > 0 && w["leadingSpace"] == true {
                            " "
                        } else {
                            ""
                        },
                        w["text"].as_str().unwrap()
                    )
                })
                .collect::<String>()
                .trim()
                .to_owned()
        };
        if text.is_empty() {
            continue;
        }
        let mut cue = json!({"id":format!("caption-{}",index+1),"startMs":start.round(),"endMs":end.round(),"text":text});
        if !words.is_empty() {
            cue["words"] = json!(words);
        }
        cues.push(cue);
    }
    Ok(cues)
}

pub fn audio_candidates(source: &Path) -> Vec<PathBuf> {
    let mut candidates = vec![];
    for track in ["mic", "system"] {
        for extension in ["m4a", "wav", "webm"] {
            let path = source.with_extension(format!("{track}.{extension}"));
            if path.is_file() {
                candidates.push(path);
            }
        }
    }
    candidates.push(source.to_path_buf());
    candidates
}

pub fn transcribe(
    source: &Path,
    runtime: &Path,
    model: &Path,
    language: &str,
    cancel: &AtomicBool,
    progress: impl Fn(&str),
) -> Result<Vec<Value>> {
    ensure!(model.is_file(), "Whisper model is missing");
    let temp = tempfile::tempdir()?;
    let wav = temp.path().join("audio.wav");
    let mut extracted = false;
    for input in audio_candidates(source) {
        ensure!(!cancel.load(Ordering::Relaxed), "Transcription cancelled");
        progress("Preparing speech audio…");
        let mut child = ManagedChild::spawn(
            Command::new(media::binary("ffmpeg")?)
                .args(["-v", "error", "-nostdin", "-y", "-i"])
                .arg(&input)
                .args(["-map", "0:a:0", "-vn", "-ar", "16000", "-ac", "1"])
                .arg(&wav),
        )?;
        if child
            .finish_cancellable(Duration::from_secs(1800), cancel)
            .is_ok()
        {
            extracted = true;
            break;
        }
    }
    ensure!(!cancel.load(Ordering::Relaxed), "Transcription cancelled");
    ensure!(
        extracted,
        "No audio track was found in the video or microphone/system companion files"
    );
    let output = temp.path().join("captions");
    progress("Transcribing locally…");
    let language = if language.trim().is_empty() {
        "auto"
    } else {
        language.trim()
    };
    let run = |full: bool| -> Result<()> {
        let mut command = Command::new(runtime);
        command
            .arg("-m")
            .arg(model)
            .arg("-f")
            .arg(&wav)
            .args(["-osrt", "-of"])
            .arg(&output)
            .args(["-l", language, "-np"]);
        if full {
            command.arg("-ojf");
        }
        let mut child = ManagedChild::spawn(command.stdout(Stdio::null()))?;
        child.finish_cancellable(Duration::from_secs(7200), cancel)
    };
    if let Err(error) = run(true) {
        let message = error.to_string();
        if !cancel.load(Ordering::Relaxed)
            && (message.contains("unknown argument") || message.contains("-ojf"))
        {
            run(false)?;
        } else {
            return Err(error);
        }
    }
    let mut cues = std::fs::read_to_string(output.with_extension("json"))
        .ok()
        .and_then(|s| parse_json(&s).ok())
        .unwrap_or_default();
    if cues.is_empty() {
        cues = parse_srt(&std::fs::read_to_string(output.with_extension("srt"))?)?;
    }
    ensure!(
        !cues.is_empty(),
        "No speech was detected; no captions were changed"
    );
    progress("Finding speech boundaries…");
    let mut detector = ManagedChild::spawn(
        Command::new(media::binary("ffmpeg")?)
            .current_dir(temp.path())
            .args(["-v", "error", "-nostdin", "-i"])
            .arg(&wav)
            .args([
                "-af",
                "silencedetect=noise=-30dB:d=0.5,ametadata=mode=print:file=silence.txt",
                "-f",
                "null",
                "-",
            ])
            .stdout(Stdio::null()),
    )?;
    // Preserve the transcript if optional analysis fails; cancellation is never a fallback.
    if detector
        .finish_cancellable(Duration::from_secs(1800), cancel)
        .is_ok()
    {
        let path = temp.path().join("silence.txt");
        if path.metadata().is_ok_and(|m| m.len() <= 64 * 1024 * 1024) {
            let log = std::fs::read_to_string(path)?;
            cues = crate::segmentation::segment(&cues, &crate::segmentation::parse_silences(&log));
        }
    }
    ensure!(!cancel.load(Ordering::Relaxed), "Transcription cancelled");
    ensure!(
        !cues.is_empty(),
        "No speech remained after silence analysis; no captions were changed"
    );
    Ok(cues)
}
