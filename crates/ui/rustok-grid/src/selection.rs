//! Row selection tracking for bulk actions.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Set of selected row ids. Ordered so serialized state is deterministic.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RowSelection {
    selected_ids: BTreeSet<String>,
}

impl RowSelection {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selected_ids.contains(id)
    }

    /// Toggle one row. Returns the new state of that row.
    pub fn toggle(&mut self, id: impl Into<String>) -> bool {
        let key = id.into();
        if self.selected_ids.remove(&key) {
            false
        } else {
            self.selected_ids.insert(key);
            true
        }
    }

    pub fn select(&mut self, id: impl Into<String>) {
        self.selected_ids.insert(id.into());
    }

    pub fn deselect(&mut self, id: &str) {
        self.selected_ids.remove(id);
    }

    pub fn set_selected(&mut self, id: impl Into<String>, selected: bool) {
        let key = id.into();
        if selected {
            self.selected_ids.insert(key);
        } else {
            self.selected_ids.remove(&key);
        }
    }

    pub fn select_all(&mut self, ids: impl IntoIterator<Item = impl Into<String>>) {
        for id in ids {
            self.selected_ids.insert(id.into());
        }
    }

    pub fn deselect_all(&mut self, ids: impl IntoIterator<Item = impl AsRef<str>>) {
        for id in ids {
            self.selected_ids.remove(id.as_ref());
        }
    }

    pub fn clear(&mut self) {
        self.selected_ids.clear();
    }

    /// Drop ids that are no longer present in the data set.
    ///
    /// Without this, a selection silently accumulates rows the user can no
    /// longer see, and bulk actions then hit records outside the current view.
    pub fn retain_ids(&mut self, ids: impl IntoIterator<Item = impl Into<String>>) {
        let keep: BTreeSet<String> = ids.into_iter().map(Into::into).collect();
        self.selected_ids.retain(|id| keep.contains(id));
    }

    /// `true` when every id in `ids` is selected. An empty input is `false`
    /// so a "select all" checkbox is never checked for an empty page.
    pub fn is_all_selected<I, S>(&self, ids: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let mut any = false;
        for id in ids {
            any = true;
            if !self.selected_ids.contains(id.as_ref()) {
                return false;
            }
        }
        any
    }

    /// `true` when some — but not all — of `ids` are selected. Adapters map
    /// this to the checkbox `indeterminate` property.
    pub fn is_partially_selected<I, S>(&self, ids: I) -> bool
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let (mut selected, mut total) = (0usize, 0usize);
        for id in ids {
            total += 1;
            if self.selected_ids.contains(id.as_ref()) {
                selected += 1;
            }
        }
        selected > 0 && selected < total
    }

    pub fn count(&self) -> usize {
        self.selected_ids.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected_ids.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = &String> {
        self.selected_ids.iter()
    }

    pub fn to_vec(&self) -> Vec<String> {
        self.selected_ids.iter().cloned().collect()
    }
}

impl<S: Into<String>> FromIterator<S> for RowSelection {
    fn from_iter<I: IntoIterator<Item = S>>(iter: I) -> Self {
        Self {
            selected_ids: iter.into_iter().map(Into::into).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_reports_the_new_state() {
        let mut selection = RowSelection::new();
        assert!(selection.toggle("a"));
        assert!(selection.is_selected("a"));
        assert!(!selection.toggle("a"));
        assert!(selection.is_empty());
    }

    #[test]
    fn all_and_partial_selection() {
        let selection: RowSelection = ["a", "b"].into_iter().collect();
        assert!(selection.is_all_selected(["a", "b"]));
        assert!(!selection.is_all_selected(["a", "b", "c"]));
        assert!(selection.is_partially_selected(["a", "c"]));
        assert!(!selection.is_partially_selected(["a", "b"]));

        // An empty page must not render as "everything selected".
        let empty: [&str; 0] = [];
        assert!(!selection.is_all_selected(empty));
    }

    #[test]
    fn retain_drops_rows_that_left_the_data_set() {
        let mut selection: RowSelection = ["a", "b", "c"].into_iter().collect();
        selection.retain_ids(["b", "c", "z"]);
        assert_eq!(selection.to_vec(), vec!["b".to_string(), "c".to_string()]);
    }
}
