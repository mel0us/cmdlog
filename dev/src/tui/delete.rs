use std::collections::HashSet;
use std::time::{Duration, Instant};

pub struct DeleteLog {
    deleted: HashSet<String>,
    undo_stack: Vec<Vec<String>>,
    pub message: Option<String>,
    message_until: Option<Instant>,
}

impl DeleteLog {
    pub fn new() -> Self {
        Self {
            deleted: HashSet::new(),
            undo_stack: Vec::new(),
            message: None,
            message_until: None,
        }
    }

    /// Insert a batch of dates. Returns count of newly-deleted entries.
    pub fn delete_batch(&mut self, dates: Vec<String>) -> usize {
        let batch: Vec<String> = dates
            .into_iter()
            .filter(|d| self.deleted.insert(d.clone()))
            .collect();
        let n = batch.len();
        if n > 0 {
            self.undo_stack.push(batch);
            self.set_message(format!(
                "Deleted {} {} (u to undo)",
                n,
                if n == 1 { "entry" } else { "entries" }
            ));
        }
        n
    }

    /// Pop the most recent batch and restore. Returns count restored.
    /// Pop the most recent batch and restore. Returns the restored dates.
    pub fn undo(&mut self) -> Vec<String> {
        if let Some(batch) = self.undo_stack.pop() {
            for key in &batch {
                self.deleted.remove(key);
            }
            self.set_message(format!("Restored {} entries", batch.len()));
            batch
        } else {
            Vec::new()
        }
    }

    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    pub fn is_deleted(&self, date: &str) -> bool {
        self.deleted.contains(date)
    }

    pub fn is_empty(&self) -> bool {
        self.deleted.is_empty()
    }

    pub fn deleted_set(&self) -> &HashSet<String> {
        &self.deleted
    }

    /// Expire timed message. Returns true if message just expired.
    pub fn tick(&mut self) -> bool {
        if let Some(until) = self.message_until {
            if Instant::now() >= until {
                self.message = None;
                self.message_until = None;
                return true;
            }
        }
        false
    }

    pub fn has_timed_message(&self) -> bool {
        self.message_until.is_some()
    }

    pub fn clear_message(&mut self) {
        self.message = None;
        self.message_until = None;
    }

    fn set_message(&mut self, msg: String) {
        self.message = Some(msg);
        self.message_until = Some(Instant::now() + Duration::from_secs(3));
    }
}
