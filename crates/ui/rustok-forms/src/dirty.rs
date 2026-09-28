//! Dirty (modified) field tracking.
//!
//! `DirtyTracker` is a framework-agnostic set-based tracker. Framework adapters
//! wrap it in their signal/state primitives. The tracker does not own field
//! values — it only records which field names have been marked dirty since the
//! last reset.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

/// Tracks which form fields have been modified since last reset.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DirtyTracker {
    fields: BTreeSet<String>,
}

impl DirtyTracker {
    pub fn new() -> Self {
        Self::default()
    }

    /// Mark a field as dirty.
    pub fn mark(&mut self, field: impl Into<String>) {
        self.fields.insert(field.into());
    }

    /// Mark a field as clean.
    pub fn unmark(&mut self, field: &str) {
        self.fields.remove(field);
    }

    /// Alias for `unmark`: mark a field as clean.
    pub fn mark_clean(&mut self, field: &str) {
        self.unmark(field);
    }

    /// Record whether a field is modified relative to its initial value.
    pub fn record_change(&mut self, field: impl Into<String>, is_different: bool) {
        let f = field.into();
        if is_different {
            self.fields.insert(f);
        } else {
            self.fields.remove(&f);
        }
    }

    /// Check whether a specific field is dirty.
    pub fn is_dirty(&self, field: &str) -> bool {
        self.fields.contains(field)
    }

    /// Check whether any field is dirty.
    pub fn is_any_dirty(&self) -> bool {
        !self.fields.is_empty()
    }

    /// Number of dirty fields.
    pub fn count(&self) -> usize {
        self.fields.len()
    }

    /// Return a vector containing all dirty field names in sorted order.
    pub fn dirty_fields(&self) -> Vec<String> {
        self.fields.iter().cloned().collect()
    }

    /// Iterate over dirty field names in sorted order.
    pub fn iter(&self) -> impl Iterator<Item = &str> {
        self.fields.iter().map(String::as_str)
    }

    /// Reset all fields to clean.
    pub fn reset(&mut self) {
        self.fields.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_and_query() {
        let mut tracker = DirtyTracker::new();
        assert!(!tracker.is_any_dirty());

        tracker.mark("title");
        tracker.mark("slug");
        assert!(tracker.is_dirty("title"));
        assert!(tracker.is_dirty("slug"));
        assert!(!tracker.is_dirty("content"));
        assert_eq!(tracker.count(), 2);
    }

    #[test]
    fn unmark_and_reset() {
        let mut tracker = DirtyTracker::new();
        tracker.mark("a");
        tracker.mark("b");
        tracker.mark_clean("a");
        assert!(!tracker.is_dirty("a"));
        assert!(tracker.is_dirty("b"));
        assert_eq!(tracker.dirty_fields(), vec!["b"]);

        tracker.reset();
        assert!(!tracker.is_any_dirty());
        assert_eq!(tracker.count(), 0);
        assert!(tracker.dirty_fields().is_empty());
    }

    #[test]
    fn record_change_marks_and_unmarks() {
        let mut tracker = DirtyTracker::new();
        tracker.record_change("title", true);
        assert!(tracker.is_dirty("title"));
        assert_eq!(tracker.dirty_fields(), vec!["title"]);

        tracker.record_change("title", false);
        assert!(!tracker.is_dirty("title"));
        assert!(tracker.dirty_fields().is_empty());
    }
}
