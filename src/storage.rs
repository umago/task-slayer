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
            return Ok(Store::default());
        }
        let bytes =
            fs::read(&self.path).with_context(|| format!("reading {}", self.path.display()))?;
        if bytes.is_empty() {
            return Ok(Store::default());
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Store, Task};

    fn temp_storage() -> (JsonStorage, tempfile::TempDir) {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("tasks.json");
        (JsonStorage::new(path), dir)
    }

    #[test]
    fn load_missing_file_returns_default() {
        let (storage, _dir) = temp_storage();
        let store = storage.load().unwrap();
        assert_eq!(store.next_id, 1);
        assert!(store.tasks.is_empty());
        assert!(!storage.path.exists(), "load should not create the file");
    }

    #[test]
    fn save_then_load_roundtrip() {
        let (storage, _dir) = temp_storage();
        let mut store = Store::default();
        store.tasks.push(Task {
            id: 1,
            description: "test".into(),
            completed: false,
            created_at: chrono::Utc::now(),
        });
        store.next_id = 2;

        storage.save(&store).unwrap();
        let loaded = storage.load().unwrap();

        assert_eq!(loaded.next_id, 2);
        assert_eq!(loaded.tasks.len(), 1);
        assert_eq!(loaded.tasks[0].description, "test");
    }

    #[test]
    fn save_creates_parent_dirs() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nested").join("deep").join("tasks.json");
        let storage = JsonStorage::new(path);

        storage.save(&Store::default()).unwrap();
        assert!(storage.path.exists());
    }

    #[test]
    fn load_parses_existing_file() {
        let (storage, _dir) = temp_storage();
        let json = r#"{
            "tasks": [
                {
                    "id": 1,
                    "description": "existing",
                    "completed": true,
                    "created_at": "2026-01-01T00:00:00Z"
                }
            ],
            "next_id": 2
        }"#;
        std::fs::write(&storage.path, json).unwrap();

        let store = storage.load().unwrap();
        assert_eq!(store.tasks.len(), 1);
        assert_eq!(store.tasks[0].id, 1);
        assert!(store.tasks[0].completed);
        assert_eq!(store.next_id, 2);
    }

    #[test]
    fn load_empty_file_returns_default() {
        let (storage, _dir) = temp_storage();
        std::fs::write(&storage.path, "").unwrap();

        let store = storage.load().unwrap();
        assert_eq!(store.next_id, 1);
        assert!(store.tasks.is_empty());
    }

    #[test]
    fn load_corrupt_file_returns_error() {
        let (storage, _dir) = temp_storage();
        std::fs::write(&storage.path, "not json").unwrap();
        assert!(storage.load().is_err());
    }

    #[test]
    fn save_is_atomic_no_tmp_left_behind() {
        let (storage, _dir) = temp_storage();
        storage.save(&Store::default()).unwrap();

        let tmp = storage.path.with_extension("json.tmp");
        assert!(!tmp.exists(), "temp file should not remain after save");
        assert!(storage.path.exists());
    }
}
