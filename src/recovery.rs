//! Ordered background snapshots never overwrite the user's saved document.
use crate::{preferences::Preferences, project::Project};
use anyhow::Result;
use serde_json::json;
use std::path::{Path, PathBuf};
pub fn directory() -> Result<PathBuf> {
    Ok(Preferences::directory()?.join("recovery"))
}

pub fn list() -> Result<Vec<PathBuf>> {
    let dir = directory()?;
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut paths = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "recordly"))
        .collect::<Vec<_>>();
    paths.sort_by_key(|p| std::cmp::Reverse(std::fs::metadata(p).and_then(|m| m.modified()).ok()));
    Ok(paths)
}

pub fn is_snapshot(path: &Path) -> bool {
    path.parent().and_then(|p| p.canonicalize().ok())
        == directory().ok().and_then(|p| p.canonicalize().ok())
        && path.is_file()
}

pub fn save(path: &Path, project: &Project, document: Option<&Path>) -> Result<()> {
    std::fs::create_dir_all(path.parent().unwrap())?;
    let mut snapshot = project.clone();
    snapshot
        .extra
        .insert("nativeRecoveryDocument".into(), json!(document));
    snapshot.save(path)?;
    let _ = std::fs::remove_file(path.with_extension("recordly.bak"));
    Ok(())
}

pub fn remove(path: &Path) -> Result<()> {
    for path in [path.to_path_buf(), path.with_extension("recordly.bak")] {
        match std::fs::remove_file(path) {
            Ok(()) => (),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

enum Operation {
    Save(Project, Option<PathBuf>),
    Clear(Option<PathBuf>),
    Flush(std::sync::mpsc::Sender<()>),
}

pub struct Store {
    sender: std::sync::mpsc::Sender<Operation>,
}

impl Store {
    pub fn new(error: impl Fn(String) + Send + 'static) -> Self {
        let (sender, receiver) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let path = directory().map(|d| d.join(format!("{}.recordly", uuid::Uuid::new_v4())));
            for op in receiver {
                let result = (|| -> Result<()> {
                    let path = path.as_ref().map_err(|e| anyhow::anyhow!("{e}"))?;
                    match op {
                        Operation::Flush(sender) => {
                            let _ = sender.send(());
                            Ok(())
                        }
                        Operation::Save(project, document) => {
                            save(path, &project, document.as_deref())
                        }
                        Operation::Clear(origin) => {
                            remove(path)?;
                            if let Some(origin) = origin
                                && is_snapshot(&origin)
                            {
                                remove(&origin)?;
                            }
                            Ok(())
                        }
                    }
                })();
                if let Err(e) = result {
                    error(format!("Recovery snapshot: {e:#}"));
                }
            }
        });
        Self { sender }
    }

    pub fn save(&self, project: Project, document: Option<PathBuf>) {
        let _ = self.sender.send(Operation::Save(project, document));
    }

    pub fn flush(&self) {
        let (tx, rx) = std::sync::mpsc::channel();
        let _ = self.sender.send(Operation::Flush(tx));
        let _ = rx.recv_timeout(std::time::Duration::from_secs(5));
    }

    pub fn clear(&self, origin: Option<PathBuf>) {
        let _ = self.sender.send(Operation::Clear(origin));
    }
}
