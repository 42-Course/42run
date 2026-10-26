//! Undo and redo by snapshots (the Memento pattern): before each change,
//! [`History::record`] keeps a copy of the state; undo swaps the current
//! state for the last copy, redo swaps it back.
//!
//! Snapshots are whole copies, which is simple and can't get out of sync
//! with the data. That's cheap for an editor's project (a few KB).
//!
//! ```
//! use pulga_engine::history::History;
//!
//! let mut doc = String::from("a");
//! let mut history = History::new(100);
//! history.record(&doc);
//! doc.push('b');
//! doc = history.undo(doc).unwrap();
//! assert_eq!(doc, "a");
//! doc = history.redo(doc).unwrap();
//! assert_eq!(doc, "ab");
//! ```

/// See the [module docs](self).
pub struct History<T> {
    undo: Vec<T>,
    redo: Vec<T>,
    limit: usize,
}

impl<T: Clone + PartialEq> History<T> {
    /// A history keeping at most `limit` undo steps (oldest dropped first).
    pub fn new(limit: usize) -> Self {
        Self {
            undo: Vec::new(),
            redo: Vec::new(),
            limit: limit.max(1),
        }
    }

    /// Remembers `state` as it is before a change. Clears redo (a new edit
    /// forks history). Recording the same state twice in a row is ignored,
    /// so callers can record eagerly.
    pub fn record(&mut self, state: &T) {
        if self.undo.last() == Some(state) {
            return;
        }
        self.undo.push(state.clone());
        if self.undo.len() > self.limit {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Steps back: returns the previous state, keeping `current` for redo.
    pub fn undo(&mut self, current: T) -> Option<T> {
        let previous = self.undo.pop()?;
        self.redo.push(current);
        Some(previous)
    }

    /// Steps forward again after an undo.
    pub fn redo(&mut self, current: T) -> Option<T> {
        let next = self.redo.pop()?;
        self.undo.push(current);
        Some(next)
    }

    /// Whether there's something to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }

    /// Whether there's something to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_edits_clear_redo() {
        let mut h = History::new(10);
        h.record(&1);
        let v = h.undo(2).unwrap();
        assert_eq!(v, 1);
        assert!(h.can_redo());
        h.record(&1);
        assert!(!h.can_redo());
    }

    #[test]
    fn limit_drops_the_oldest_and_duplicates_are_ignored() {
        let mut h = History::new(2);
        for s in [1, 1, 2, 3] {
            h.record(&s);
        }
        assert_eq!(h.undo(4), Some(3));
        assert_eq!(h.undo(3), Some(2));
        assert_eq!(h.undo(2), None);
    }
}
