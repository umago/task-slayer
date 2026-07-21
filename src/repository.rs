//! Task repository: domain operations over the store.
//!
/// Commands talk to this layer, never to the JSON file directly.
use anyhow::{Result, anyhow};

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
        let description: String = description.into();
        if description.trim().is_empty() {
            return Err(anyhow!("description cannot be empty"));
        }
        let mut store = self.load()?;
        let id = store.next_id;
        store.next_id = id
            .checked_add(1)
            .ok_or_else(|| anyhow!("ID counter overflow"))?;
        let task = Task {
            id,
            description,
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

    // Defensive guard: the CLI deduplicates via expand_selectors, but direct
    // callers (tests, future API) may not.
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
        if touched.len() != target.len() {
            let missing: Vec<String> = target
                .iter()
                .filter(|id| !touched.contains(id))
                .map(|id| id.to_string())
                .collect();
            return Err(anyhow!("task ids not found: {}", missing.join(", ")));
        }
        self.save(&store)?;
        Ok(touched)
    }

    pub fn set_completed(&self, ids: &[u64], completed: bool) -> Result<Vec<u64>> {
        self.mutate(ids, |t| t.completed = completed)
    }

    /// Replace a single task's description.
    ///
    /// Unlike `set_completed`/`remove`, `edit` targets exactly one task — a
    /// new description is meaningless to apply across many. Unknown ids
    /// produce `task id not found: N`, matching the wording used by `mutate`.
    pub fn edit_task(&self, id: u64, description: impl Into<String>) -> Result<Task> {
        let description: String = description.into();
        if description.trim().is_empty() {
            return Err(anyhow!("description cannot be empty"));
        }
        let mut store = self.load()?;
        let task = store
            .tasks
            .iter_mut()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow!("task id not found: {id}"))?;
        task.description = description;
        let updated = task.clone();
        self.save(&store)?;
        Ok(updated)
    }

    /// Renumber all tasks sequentially starting from 1, preserving their
    /// relative order.  Returns the number of tasks that were renumbered
    /// (0 when the store is empty — nothing to compact).
    pub fn compact(&self) -> Result<usize> {
        let mut store = self.load()?;
        if store.tasks.is_empty() {
            return Ok(0);
        }
        store.tasks.sort_by_key(|t| t.id);
        for (i, task) in store.tasks.iter_mut().enumerate() {
            task.id = (i as u64) + 1;
        }
        store.next_id = (store.tasks.len() as u64) + 1;
        self.save(&store)?;
        Ok(store.tasks.len())
    }

    pub fn remove(&self, ids: &[u64]) -> Result<Vec<u64>> {
        let target = self.resolve_ids(ids);
        if target.is_empty() {
            return Err(anyhow!("no task ids given"));
        }
        let mut store = self.load()?;
        let mut touched = Vec::new();
        store.tasks.retain(|t| {
            if target.binary_search(&t.id).is_ok() {
                touched.push(t.id);
                false
            } else {
                true
            }
        });
        if touched.is_empty() {
            return Err(anyhow!("no tasks matched the given ids"));
        }
        if touched.len() != target.len() {
            let missing: Vec<String> = target
                .iter()
                .filter(|id| !touched.contains(id))
                .map(|id| id.to_string())
                .collect();
            return Err(anyhow!("task ids not found: {}", missing.join(", ")));
        }
        self.save(&store)?;
        Ok(touched)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Store;
    use std::cell::RefCell;

    /// In-memory storage for testing the repository without touching disk.
    struct MemStorage {
        store: RefCell<Store>,
    }

    impl MemStorage {
        fn new() -> Self {
            Self {
                store: RefCell::new(Store::default()),
            }
        }
    }

    impl Storage for MemStorage {
        fn load(&self) -> Result<Store> {
            Ok(self.store.borrow().clone())
        }
        fn save(&self, store: &Store) -> Result<()> {
            *self.store.borrow_mut() = store.clone();
            Ok(())
        }
    }

    fn repo() -> TaskRepository<MemStorage> {
        TaskRepository::new(MemStorage::new())
    }

    #[test]
    fn add_assigns_incrementing_ids() {
        let r = repo();
        let t1 = r.add("first").unwrap();
        let t2 = r.add("second").unwrap();
        let t3 = r.add("third").unwrap();
        assert_eq!(t1.id, 1);
        assert_eq!(t2.id, 2);
        assert_eq!(t3.id, 3);
    }

    #[test]
    fn add_stores_description() {
        let r = repo();
        let t = r.add("buy milk").unwrap();
        assert_eq!(t.description, "buy milk");
        assert!(!t.completed);
    }

    #[test]
    fn list_pending_excludes_completed() {
        let r = repo();
        r.add("a").unwrap();
        r.add("b").unwrap();
        r.add("c").unwrap();
        r.set_completed(&[2], true).unwrap();

        let pending = r.list_pending().unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].id, 1);
        assert_eq!(pending[1].id, 3);
    }

    #[test]
    fn list_all_includes_completed() {
        let r = repo();
        r.add("a").unwrap();
        r.add("b").unwrap();
        r.set_completed(&[1], true).unwrap();

        let all = r.list_all().unwrap();
        assert_eq!(all.len(), 2);
        assert!(all[0].completed);
        assert!(!all[1].completed);
    }

    #[test]
    fn done_marks_completed() {
        let r = repo();
        r.add("a").unwrap();
        r.add("b").unwrap();

        let touched = r.set_completed(&[1], true).unwrap();
        assert_eq!(touched, vec![1]);

        let all = r.list_all().unwrap();
        assert!(all[0].completed);
        assert!(!all[1].completed);
    }

    #[test]
    fn undo_marks_pending() {
        let r = repo();
        r.add("a").unwrap();
        r.set_completed(&[1], true).unwrap();
        r.set_completed(&[1], false).unwrap();

        let pending = r.list_pending().unwrap();
        assert_eq!(pending.len(), 1);
    }

    #[test]
    fn rm_deletes_tasks() {
        let r = repo();
        r.add("a").unwrap();
        r.add("b").unwrap();
        r.add("c").unwrap();

        let touched = r.remove(&[2]).unwrap();
        assert_eq!(touched.len(), 1);

        let all = r.list_all().unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].id, 1);
        assert_eq!(all[1].id, 3);
    }

    #[test]
    fn edit_updates_description() {
        let r = repo();
        r.add("original").unwrap();

        let updated = r.edit_task(1, "revised").unwrap();
        assert_eq!(updated.id, 1);
        assert_eq!(updated.description, "revised");
        assert!(!updated.completed, "edit must not touch completed");

        let all = r.list_all().unwrap();
        assert_eq!(all[0].description, "revised");
    }

    #[test]
    fn edit_nonexistent_returns_error() {
        let r = repo();
        r.add("a").unwrap();
        assert!(r.edit_task(99, "x").is_err());
    }

    #[test]
    fn ids_not_reused_after_rm() {
        let r = repo();
        r.add("a").unwrap(); // id 1
        r.add("b").unwrap(); // id 2
        r.remove(&[1]).unwrap();
        r.remove(&[2]).unwrap();

        let t = r.add("c").unwrap();
        assert_eq!(t.id, 3, "id should continue from next_id, not restart");
    }

    #[test]
    fn ids_not_reused_after_rm_all() {
        let r = repo();
        r.add("a").unwrap();
        r.add("b").unwrap();
        r.remove(&[1, 2]).unwrap();

        // Store is empty but next_id should still be 3
        let t = r.add("c").unwrap();
        assert_eq!(t.id, 3);
    }

    #[test]
    fn rm_nonexistent_returns_error() {
        let r = repo();
        r.add("a").unwrap();
        assert!(r.remove(&[99]).is_err());
    }

    #[test]
    fn done_nonexistent_returns_error() {
        let r = repo();
        r.add("a").unwrap();
        assert!(r.set_completed(&[99], true).is_err());
    }

    #[test]
    fn empty_selector_returns_error() {
        let r = repo();
        r.add("a").unwrap();
        assert!(r.remove(&[]).is_err());
        assert!(r.set_completed(&[], true).is_err());
    }

    #[test]
    fn done_range_marks_multiple() {
        let r = repo();
        for i in 1..=5 {
            r.add(format!("task {i}")).unwrap();
        }
        let touched = r.set_completed(&[2, 3, 4], true).unwrap();
        assert_eq!(touched.len(), 3);

        let pending = r.list_pending().unwrap();
        assert_eq!(pending.len(), 2);
        assert_eq!(pending[0].id, 1);
        assert_eq!(pending[1].id, 5);
    }

    #[test]
    fn compact_renumbers_tasks() {
        let r = repo();
        r.add("a").unwrap(); // 1
        r.add("b").unwrap(); // 2
        r.add("c").unwrap(); // 3
        r.remove(&[2]).unwrap();

        let n = r.compact().unwrap();
        assert_eq!(n, 2);

        let all = r.list_all().unwrap();
        assert_eq!(all[0].id, 1);
        assert_eq!(all[0].description, "a");
        assert_eq!(all[1].id, 2);
        assert_eq!(all[1].description, "c");
    }

    #[test]
    fn compact_resets_next_id() {
        let r = repo();
        r.add("a").unwrap();
        r.add("b").unwrap();
        r.add("c").unwrap();
        r.remove(&[1, 2]).unwrap();

        r.compact().unwrap();

        let t = r.add("d").unwrap();
        assert_eq!(
            t.id, 2,
            "next add after compact should use next_id = len + 1"
        );
    }

    #[test]
    fn compact_preserves_order() {
        let r = repo();
        for i in 1..=5 {
            r.add(format!("task {i}")).unwrap();
        }
        r.remove(&[2, 4]).unwrap();

        r.compact().unwrap();

        let all = r.list_all().unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].id, 1);
        assert_eq!(all[0].description, "task 1");
        assert_eq!(all[1].id, 2);
        assert_eq!(all[1].description, "task 3");
        assert_eq!(all[2].id, 3);
        assert_eq!(all[2].description, "task 5");
    }

    #[test]
    fn compact_empty_store_returns_zero() {
        let r = repo();
        let n = r.compact().unwrap();
        assert_eq!(n, 0);

        // next_id should remain 1
        let t = r.add("first").unwrap();
        assert_eq!(t.id, 1);
    }

    #[test]
    fn compact_already_sequential_is_noop() {
        let r = repo();
        r.add("a").unwrap();
        r.add("b").unwrap();
        r.add("c").unwrap();

        let n = r.compact().unwrap();
        assert_eq!(n, 3);

        let all = r.list_all().unwrap();
        assert_eq!(all[0].id, 1);
        assert_eq!(all[1].id, 2);
        assert_eq!(all[2].id, 3);

        let t = r.add("d").unwrap();
        assert_eq!(t.id, 4);
    }
}
