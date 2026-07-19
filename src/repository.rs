//! Task repository: domain operations over the store.
//!
/// Commands talk to this layer, never to the JSON file directly.
use anyhow::{anyhow, Result};

use crate::model::{Store, Task};
use crate::storage::Storage;

pub struct TaskRepository<S: Storage> {
    storage: S,
}

impl<S: Storage> TaskRepository<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    fn load(&self) -> Result<Store> {
        self.storage.load()
    }

    fn save(&self, store: &Store) -> Result<()> {
        self.storage.save(store)
    }

    pub fn add(&self, description: impl Into<String>) -> Result<Task> {
        let mut store = self.load()?;
        let id = store.next_id;
        store.next_id = id.checked_add(1).ok_or_else(|| anyhow!("ID counter overflow"))?;
        let task = Task {
            id,
            description: description.into(),
            completed: false,
            created_at: chrono::Utc::now(),
        };
        store.tasks.push(task.clone());
        self.save(&store)?;
        Ok(task)
    }

    pub fn list_pending(&self) -> Result<Vec<Task>> {
        let store = self.load()?;
        let mut tasks: Vec<Task> = store.tasks.into_iter().filter(|t| !t.completed).collect();
        tasks.sort_by_key(|t| t.id);
        Ok(tasks)
    }

    pub fn list_all(&self) -> Result<Vec<Task>> {
        let store = self.load()?;
        let mut tasks = store.tasks;
        tasks.sort_by_key(|t| t.id);
        Ok(tasks)
    }

    fn resolve_ids(&self, ids: &[u64]) -> Vec<u64> {
        let mut v: Vec<u64> = ids.to_vec();
        v.sort_unstable();
        v.dedup();
        v
    }

    /// Apply a mutation to each task whose id appears in `ids`.
    /// Returns the ids that were actually present in the store.
    fn mutate<F>(&self, ids: &[u64], mut f: F) -> Result<Vec<u64>>
    where
        F: FnMut(&mut Task),
    {
        let target = self.resolve_ids(ids);
        if target.is_empty() {
            return Err(anyhow!("no task ids given"));
        }
        let mut store = self.load()?;
        let mut touched = Vec::new();
        for task in store.tasks.iter_mut() {
            if target.binary_search(&task.id).is_ok() {
                f(task);
                touched.push(task.id);
            }
        }
        if touched.is_empty() {
            return Err(anyhow!("no tasks matched the given ids"));
        }
        self.save(&store)?;
        Ok(touched)
    }

    pub fn set_completed(&self, ids: &[u64], completed: bool) -> Result<Vec<u64>> {
        self.mutate(ids, |t| t.completed = completed)
    }

    pub fn remove(&self, ids: &[u64]) -> Result<Vec<u64>> {
        let target = self.resolve_ids(ids);
        if target.is_empty() {
            return Err(anyhow!("no task ids given"));
        }
        let mut store = self.load()?;
        let before = store.tasks.len();
        store.tasks.retain(|t| target.binary_search(&t.id).is_err());
        let removed = before - store.tasks.len();
        if removed == 0 {
            return Err(anyhow!("no tasks matched the given ids"));
        }
        self.save(&store)?;
        Ok(target.iter().copied().take(removed).collect())
    }
}
