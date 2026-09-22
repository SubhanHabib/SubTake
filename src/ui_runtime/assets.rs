//! Resolves the view's asset paths to bundled files.

use super::*;

pub(super) struct Assets;
impl gpui::AssetSource for Assets {
    fn load(&self, path: &str) -> Result<Option<std::borrow::Cow<'static, [u8]>>> {
        let relative = Path::new(path);
        // App assets are trusted paths supplied by views, never project contents.
        let path = if relative.is_absolute() {
            relative.to_path_buf()
        } else {
            crate::media::resources()
                .join(relative.strip_prefix("legacy-electron").unwrap_or(relative))
        };
        match std::fs::read(&path) {
            Ok(bytes) => Ok(Some(std::borrow::Cow::Owned(bytes))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                let local = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
                Ok(std::fs::read(local).ok().map(std::borrow::Cow::Owned))
            }
            Err(e) => Err(e.into()),
        }
    }
    fn list(&self, path: &str) -> Result<Vec<gpui::SharedString>> {
        let directory = crate::media::resources().join(path);
        if !directory.is_dir() {
            return Ok(vec![]);
        }
        Ok(std::fs::read_dir(directory)?
            .filter_map(|entry| {
                entry
                    .ok()
                    .map(|e| e.path().to_string_lossy().to_string().into())
            })
            .collect())
    }
}
