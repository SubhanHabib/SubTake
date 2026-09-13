//! Replace related recording files together, retaining rollback data on failure.
//! The caller owns `work` and must retain it if this function returns an error.
use anyhow::{Context, Result, ensure};
use std::{
    io::Write,
    path::{Path, PathBuf},
};
pub fn replace(work: &Path, files: &[(PathBuf, Option<PathBuf>)]) -> Result<()> {
    let backup = work.join("previous-recording");
    std::fs::create_dir_all(&backup)?;
    for (destination, source) in files {
        ensure!(
            !destination.is_dir(),
            "Recording destination is a directory: {}",
            destination.display()
        );
        if let Some(source) = source {
            ensure!(
                source.is_file(),
                "Recording component is missing: {}",
                source.display()
            );
        }
    }
    std::fs::write(work.join("commit.json"), serde_json::to_vec_pretty(files)?)?;
    let mut moved: Vec<(PathBuf, PathBuf)> = vec![];
    let mut installed: Vec<PathBuf> = vec![];
    let result = (|| -> Result<()> {
        for (index, (destination, _)) in files.iter().enumerate() {
            if destination.exists() {
                let old = backup.join(index.to_string());
                std::fs::rename(destination, &old)?;
                moved.push((destination.clone(), old));
            }
        }
        for (destination, source) in files {
            if let Some(source) = source {
                let mut file = tempfile::NamedTempFile::new_in(
                    destination.parent().unwrap_or(Path::new(".")),
                )?;
                std::io::copy(&mut std::fs::File::open(source)?, &mut file)?;
                file.flush()?;
                file.as_file().sync_all()?;
                file.persist(destination).map_err(|e| e.error)?;
                installed.push(destination.clone());
            }
        }
        Ok(())
    })();
    if let Err(error) = result {
        let mut rollback_errors = vec![];
        for path in installed {
            if let Err(e) = std::fs::remove_file(&path) {
                rollback_errors.push(format!("{}: {e}", path.display()));
            }
        }
        for (destination, old) in moved {
            if let Err(e) = std::fs::rename(&old, &destination) {
                rollback_errors.push(format!("{}: {e}", destination.display()));
            }
        }
        return Err(error.context(if rollback_errors.is_empty() {
            "Recording replacement was rolled back".into()
        } else {
            format!(
                "Rollback needs recovery from {}: {}",
                work.display(),
                rollback_errors.join("; ")
            )
        }));
    }
    #[cfg(unix)]
    if let Some((destination, _)) = files.first() {
        std::fs::File::open(destination.parent().unwrap_or(Path::new(".")))?
            .sync_all()
            .context("Sync recording folder")?;
    }
    Ok(())
}
