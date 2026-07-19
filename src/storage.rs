//! JSON-backed storage with atomic writes.
//!
/// The storage trait is intentionally narrow so that the backend can be
/// swapped (e.g. SQLite) without touching command logic.
use std::fs;
use std::io::Write;
use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::model::Store;

pub trait Storage {
    fn load(&self) -> Result<Store>;
    fn save(&self, store: &Store) -> Result<()>;
}

pub struct JsonStorage {
    path: PathBuf,
}

impl JsonStorage {
    /// Resolve the storage path from `XDG_DATA_HOME`, falling back to
    /// `~/.local/share/tslay/tasks.json` per the spec.
    pub fn default_path() -> Result<PathBuf> {
        let dir = match std::env::var("XDG_DATA_HOME") {
            Ok(v) if !v.is_empty() => PathBuf::from(v),
            _ => {
                let home = std::env::var("HOME").context("HOME is not set")?;
                PathBuf::from(home).join(".local").join("share")
            }
        };
        Ok(dir.join("tslay").join("tasks.json"))
    }

    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn ensure_dir(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("creating directory {}", parent.display()))?;
        }
        Ok(())
    }
}

impl Storage for JsonStorage {
    fn load(&self) -> Result<Store> {
        if !self.path.exists() {
            // Spec: create the directory and file automatically if they
            // do not exist, even for read-only commands.
            self.save(&Store::default())?;
            return Ok(Store::default());
        }
        let bytes = fs::read(&self.path)
            .with_context(|| format!("reading {}", self.path.display()))?;
        let store: Store = serde_json::from_slice(&bytes)
            .with_context(|| format!("parsing {}", self.path.display()))?;
        Ok(store)
    }

    fn save(&self, store: &Store) -> Result<()> {
        self.ensure_dir()?;
        let json = serde_json::to_vec_pretty(store).context("serializing store")?;

        // Atomic write: write to a temp file in the same directory, fsync, then
        // rename over the original.
        let tmp = self.path.with_extension("json.tmp");
        {
            let mut file = fs::File::create(&tmp)
                .with_context(|| format!("creating temp file {}", tmp.display()))?;
            file.write_all(&json).context("writing temp file")?;
            file.sync_all().context("fsyncing temp file")?;
        }
        fs::rename(&tmp, &self.path)
            .with_context(|| format!("replacing {}", self.path.display()))?;
        Ok(())
    }
}

/// Convenience constructor used by the CLI.
pub fn default_storage() -> Result<impl Storage> {
    Ok(JsonStorage::new(JsonStorage::default_path()?))
}
