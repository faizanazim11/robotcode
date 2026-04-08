//! In-memory REPL history store.
//!
//! Tracks the list of evaluated expressions in the current REPL session,
//! bounded by a configurable maximum capacity. Entries are kept in
//! insertion order; the most recent entry is last.

/// Maximum number of history entries retained by default.
const DEFAULT_CAPACITY: usize = 500;

/// A single history entry recording one REPL evaluation.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct HistoryEntry {
    /// Sequential entry number (1-based).
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
    entries: std::sync::Mutex<Vec<HistoryEntry>>,
    capacity: usize,
}

impl History {
    /// Create a new history store with the default capacity.
    pub fn new() -> Self {
        Self {
            entries: std::sync::Mutex::new(Vec::new()),
            capacity: DEFAULT_CAPACITY,
        }
    }

    /// Create a new history store with a custom capacity.
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            entries: std::sync::Mutex::new(Vec::new()),
            capacity,
        }
    }

    /// Append a new entry. Returns the assigned index.
    pub fn push(&self, expression: String, result: Option<String>, error: bool) -> usize {
        let mut entries = self.entries.lock().expect("history mutex poisoned");
        let index = entries.len() + 1;
        if entries.len() >= self.capacity {
            entries.remove(0);
        }
        entries.push(HistoryEntry {
            index,
            expression,
            result,
            error,
        });
        index
    }

    /// Return a snapshot of all history entries.
    pub fn entries(&self) -> Vec<HistoryEntry> {
        self.entries.lock().expect("history mutex poisoned").clone()
    }

    /// Clear all history entries.
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
    fn clear() {
        let h = History::new();
        h.push("Log  hi".into(), None, false);
        h.clear();
        assert!(h.entries().is_empty());
    }
}
