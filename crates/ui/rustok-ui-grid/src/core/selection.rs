use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

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

    pub fn toggle(&mut self, id: impl Into<String>) {
        let key = id.into();
        if self.selected_ids.contains(&key) {
            self.selected_ids.remove(&key);
        } else {
            self.selected_ids.insert(key);
        }
    }

    pub fn select(&mut self, id: impl Into<String>) {
        self.selected_ids.insert(id.into());
    }

    pub fn deselect(&mut self, id: &str) {
        self.selected_ids.remove(id);
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
