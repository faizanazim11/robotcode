//! In-memory REPL history store.
//!
//! Tracks the list of evaluated expressions in the current REPL session,
//! bounded by a configurable maximum capacity. Entries are kept in
//! insertion order; the most recent entry is last.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;

/// Maximum number of history entries retained by default.
const DEFAULT_CAPACITY: usize = 500;

/// A single history entry recording one REPL evaluation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    /// Monotonically increasing entry number (1-based, never resets on eviction).
    pub index: usize,
    /// The expression / keyword call that was evaluated.
    pub expression: String,
    /// The result returned by the evaluation, if any.
    pub result: Option<String>,
    /// Whether the evaluation produced an error.
    pub error: bool,
}

/// Thread-safe REPL history store.
#[derive(Debug)]
pub struct History {
    entries: Mutex<VecDeque<HistoryEntry>>,
    capacity: usize,
    /// Monotonic counter; never decremented even when entries are evicted.
    next_index: AtomicUsize,
}

impl History {
    /// Create a new history store with the default capacity.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            capacity: DEFAULT_CAPACITY,
            next_index: AtomicUsize::new(1),
        }
    }

    /// Create a new history store with a custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: Mutex::new(VecDeque::new()),
            capacity,
            next_index: AtomicUsize::new(1),
        }
    }

    /// Append a new entry. Returns the assigned (monotonic) index.
    pub fn push(&self, expression: String, result: Option<String>, error: bool) -> usize {
        let index = self.next_index.fetch_add(1, Ordering::Relaxed);
        let mut entries = self.entries.lock().expect("history mutex poisoned");
        if entries.len() >= self.capacity {
            entries.pop_front();
        }
        entries.push_back(HistoryEntry {
            index,
            expression,
            result,
            error,
        });
        index
    }

    /// Return a snapshot of all history entries.
    pub fn entries(&self) -> Vec<HistoryEntry> {
        self.entries
            .lock()
            .expect("history mutex poisoned")
            .iter()
            .cloned()
            .collect()
    }

    /// Clear all history entries (the monotonic counter is preserved).
    pub fn clear(&self) {
        self.entries.lock().expect("history mutex poisoned").clear();
    }
}

impl Default for History {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn push_and_retrieve() {
        let h = History::new();
        let i1 = h.push("Log  hello".into(), Some("None".into()), false);
        let i2 = h.push("Fail  boom".into(), None, true);
        assert_eq!(i1, 1);
        assert_eq!(i2, 2);
        let entries = h.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].expression, "Log  hello");
        assert!(!entries[0].error);
        assert!(entries[1].error);
    }

    #[test]
    fn capacity_eviction() {
        let h = History::with_capacity(3);
        for i in 0..5 {
            h.push(format!("kw{i}"), None, false);
        }
        let entries = h.entries();
        // Only the last 3 entries are kept.
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0].expression, "kw2");
        assert_eq!(entries[2].expression, "kw4");
    }

    #[test]
    fn monotonic_index_after_eviction() {
        // Indexes must keep incrementing even after entries are evicted.
        let h = History::with_capacity(2);
        let i1 = h.push("kw1".into(), None, false);
        let i2 = h.push("kw2".into(), None, false);
        let i3 = h.push("kw3".into(), None, false); // evicts kw1
        assert_eq!(i1, 1);
        assert_eq!(i2, 2);
        assert_eq!(i3, 3);
        let entries = h.entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].index, 2);
        assert_eq!(entries[1].index, 3);
    }

    #[test]
    fn clear() {
        let h = History::new();
        h.push("Log  hi".into(), None, false);
        h.clear();
        assert!(h.entries().is_empty());
    }
}
