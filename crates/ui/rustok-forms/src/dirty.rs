//! Dirty (modified) field tracking.
//!
//! `DirtyTracker` is a framework-agnostic set-based tracker. Framework adapters
//! wrap it in their signal/state primitives. The tracker does not own field
//! values — it only records which field names have been marked dirty since the
//! last reset.

use std::collections::BTreeSet;
use std::hash::Hash;

use serde::{Deserialize, Serialize};

/// Tracks which form fields have been modified since last reset.
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DirtyTracker {
    fields: BTreeSet<String>,
}

impl DirtyTracker {
    /// Create a new empty `DirtyTracker`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Construct a `DirtyTracker` pre-populated with an initial collection of field names.
    pub fn from_fields<I, S>(fields: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut tracker = Self::new();
        tracker.mark_many(fields);
        tracker
    }

    /// Mark a field as dirty.
    pub fn mark(&mut self, field: impl Into<String>) {
        self.fields.insert(field.into());
    }

    /// Mark multiple fields as dirty.
    pub fn mark_many<I, S>(&mut self, fields: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        for f in fields {
            self.fields.insert(f.into());
        }
    }

    /// Mark a field as clean.
    pub fn unmark(&mut self, field: &str) {
        self.fields.remove(field);
    }

    /// Mark multiple fields as clean.
    pub fn unmark_many<'a, I>(&mut self, fields: I)
    where
        I: IntoIterator<Item = &'a str>,
    {
        for f in fields {
            self.fields.remove(f);
        }
    }

    /// Alias for `unmark`: mark a field as clean.
    pub fn mark_clean(&mut self, field: &str) {
        self.unmark(field);
    }

    /// Record whether a field is modified relative to its initial value.
    pub fn record_change(&mut self, field: impl AsRef<str>, is_different: bool) {
        let f = field.as_ref();
        if is_different {
            self.fields.insert(f.to_string());
        } else {
            self.fields.remove(f);
        }
    }

    /// Check whether a specific field is dirty.
    pub fn is_dirty(&self, field: &str) -> bool {
        self.fields.contains(field)
    }

    /// Alias for `is_dirty`.
    pub fn contains(&self, field: &str) -> bool {
        self.is_dirty(field)
    }

    /// Check whether any of the specified fields are dirty.
    pub fn is_any_of_dirty(&self, fields: &[&str]) -> bool {
        fields.iter().any(|f| self.is_dirty(f))
    }

    /// Check whether all of the specified fields are dirty.
    pub fn are_all_dirty(&self, fields: &[&str]) -> bool {
        !fields.is_empty() && fields.iter().all(|f| self.is_dirty(f))
    }

    /// Check whether any field is dirty.
    pub fn is_any_dirty(&self) -> bool {
        !self.fields.is_empty()
    }

    /// Check whether no fields are dirty.
    pub fn is_empty(&self) -> bool {
        self.fields.is_empty()
    }

    /// Number of dirty fields.
    pub fn count(&self) -> usize {
        self.fields.len()
    }

    /// Alias for `count()`.
    pub fn len(&self) -> usize {
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

    /// Retain only the dirty fields that satisfy the predicate.
    pub fn retain<F>(&mut self, mut f: F)
    where
        F: FnMut(&str) -> bool,
    {
        self.fields.retain(|field| f(field.as_str()));
    }

    /// Reset all fields to clean.
    pub fn reset(&mut self) {
        self.fields.clear();
    }

    /// Alias for `reset()`.
    pub fn clear(&mut self) {
        self.reset();
    }
}

impl Extend<String> for DirtyTracker {
    fn extend<T: IntoIterator<Item = String>>(&mut self, iter: T) {
        self.fields.extend(iter);
    }
}

impl<'a> Extend<&'a str> for DirtyTracker {
    fn extend<T: IntoIterator<Item = &'a str>>(&mut self, iter: T) {
        self.fields.extend(iter.into_iter().map(String::from));
    }
}

impl FromIterator<String> for DirtyTracker {
    fn from_iter<T: IntoIterator<Item = String>>(iter: T) -> Self {
        Self {
            fields: iter.into_iter().collect(),
        }
    }
}

impl<'a> FromIterator<&'a str> for DirtyTracker {
    fn from_iter<T: IntoIterator<Item = &'a str>>(iter: T) -> Self {
        Self {
            fields: iter.into_iter().map(String::from).collect(),
        }
    }
}

impl IntoIterator for DirtyTracker {
    type Item = String;
    type IntoIter = std::collections::btree_set::IntoIter<String>;

    fn into_iter(self) -> Self::IntoIter {
        self.fields.into_iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mark_and_query() {
        let mut tracker = DirtyTracker::new();
        assert!(!tracker.is_any_dirty());
        assert!(tracker.is_empty());
        assert_eq!(tracker.len(), 0);

        tracker.mark("title");
        tracker.mark("slug");
        assert!(tracker.is_dirty("title"));
        assert!(tracker.contains("slug"));
        assert!(!tracker.is_dirty("content"));
        assert_eq!(tracker.count(), 2);
        assert_eq!(tracker.len(), 2);
    }

    #[test]
    fn mark_many_and_unmark_many() {
        let mut tracker = DirtyTracker::new();
        tracker.mark_many(["a", "b", "c"]);
        assert_eq!(tracker.count(), 3);

        tracker.unmark_many(["a", "c"]);
        assert_eq!(tracker.dirty_fields(), vec!["b"]);
    }

    #[test]
    fn is_any_of_and_are_all_dirty() {
        let tracker = DirtyTracker::from_fields(["title", "slug"]);
        assert!(tracker.is_any_of_dirty(&["title", "content"]));
        assert!(!tracker.is_any_of_dirty(&["author", "content"]));
        assert!(tracker.are_all_dirty(&["title", "slug"]));
        assert!(!tracker.are_all_dirty(&["title", "content"]));
    }

    #[test]
    fn from_fields_and_iter() {
        let tracker = DirtyTracker::from_fields(["slug", "title"]);
        let fields: Vec<&str> = tracker.iter().collect();
        assert_eq!(fields, vec!["slug", "title"]);
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

    #[test]
    fn from_iterator_and_extend() {
        let mut tracker: DirtyTracker = ["first", "second"].into_iter().collect();
        assert_eq!(tracker.len(), 2);

        tracker.extend(["third", "fourth"]);
        assert_eq!(tracker.len(), 4);
    }
}
