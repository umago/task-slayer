use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A single task.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: u64,
    pub description: String,
    pub completed: bool,
    pub created_at: DateTime<Utc>,
}

/// On-disk container.
///
/// The counter is persisted separately from the task list so that IDs are never
/// reused, even after all tasks have been deleted (see persistence rules).
/// A bare array would lose the high-water mark once the list becomes empty.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Store {
    pub tasks: Vec<Task>,
    pub next_id: u64,
}

impl Default for Store {
    /// IDs start at 1; `next_id` is the id that will be assigned next.
    fn default() -> Self {
        Self {
            tasks: Vec::new(),
            next_id: 1,
        }
    }
}
