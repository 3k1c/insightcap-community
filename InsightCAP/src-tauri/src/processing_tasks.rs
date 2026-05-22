use std::collections::HashSet;
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
pub struct ProcessingTaskState {
    cancelled: Arc<Mutex<HashSet<String>>>,
}

impl ProcessingTaskState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self, task_id: &str) {
        if let Ok(mut cancelled) = self.cancelled.lock() {
            cancelled.insert(task_id.to_string());
        }
    }

    pub fn is_cancelled(&self, task_id: &str) -> bool {
        self.cancelled
            .lock()
            .map(|cancelled| cancelled.contains(task_id))
            .unwrap_or(false)
    }

    pub fn clear(&self, task_id: &str) {
        if let Ok(mut cancelled) = self.cancelled.lock() {
            cancelled.remove(task_id);
        }
    }
}
