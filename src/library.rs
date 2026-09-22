//! A shared, read-only index of the chosen project folder and recent files.
use anyhow::Result;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

pub fn entries(folder: Option<&Path>, recent: &[PathBuf], query: &str) -> Result<Vec<PathBuf>> {
    let mut candidates = recent.to_vec();
    if let Some(folder) = folder {
        let mut files = std::fs::read_dir(folder)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.is_file()
                    && matches!(
                        p.extension()
                            .and_then(|s| s.to_str())
                            .map(str::to_lowercase)
                            .as_deref(),
                        Some("recordly" | "openscreen" | "mp4" | "mov" | "mkv" | "webm")
                    )
                    && !p
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_lowercase()
                        .ends_with(".webcam.mp4")
            })
            .collect::<Vec<_>>();
        files.sort_by_key(|p| std::cmp::Reverse(p.metadata().and_then(|m| m.modified()).ok()));
        candidates.extend(files);
    }
    let query = query.to_lowercase();
    let mut seen = HashSet::new();
    candidates.retain(|p| {
        p.is_file()
            && p.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase()
                .contains(&query)
            && seen.insert(p.canonicalize().unwrap_or_else(|_| p.clone()))
    });
    candidates.truncate(256);
    Ok(candidates)
}
