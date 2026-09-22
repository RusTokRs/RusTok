/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

use std::collections::BTreeSet;
use serde::{Deserialize, Serialize};

/// Framework-agnostic row and item selection state for table and list views.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiSelectionState<ID: Ord = String> {
    pub selected: BTreeSet<ID>,
}

impl<ID: Ord> UiSelectionState<ID> {
    pub fn new() -> Self {
        Self {
            selected: BTreeSet::new(),
        }
    }

    pub fn from_ids(ids: impl IntoIterator<Item = ID>) -> Self {
        Self {
            selected: ids.into_iter().collect(),
        }
    }

    pub fn is_selected(&self, id: &ID) -> bool {
        self.selected.contains(id)
    }

    pub fn select(&mut self, id: ID) -> bool {
        self.selected.insert(id)
    }

    pub fn deselect(&mut self, id: &ID) -> bool {
        self.selected.remove(id)
    }

    pub fn toggle(&mut self, id: ID) -> bool {
        if self.selected.contains(&id) {
            self.selected.remove(&id);
            false
        } else {
            self.selected.insert(id);
            true
        }
    }

    pub fn select_all(&mut self, ids: impl IntoIterator<Item = ID>) {
        self.selected.extend(ids);
    }

    pub fn deselect_all(&mut self, ids: impl IntoIterator<Item = ID>) {
        for id in ids {
            self.selected.remove(&id);
        }
    }

    pub fn clear(&mut self) {
        self.selected.clear();
    }

    pub fn count(&self) -> usize {
        self.selected.len()
    }

    pub fn is_empty(&self) -> bool {
        self.selected.is_empty()
    }

    pub fn is_all_selected<'a>(&self, ids: impl IntoIterator<Item = &'a ID>) -> bool
    where
        ID: 'a,
    {
        let mut any = false;
        for id in ids {
            any = true;
            if !self.selected.contains(id) {
                return false;
            }
        }
        any
    }

    pub fn is_partially_selected<'a>(&self, ids: impl IntoIterator<Item = &'a ID>) -> bool
    where
        ID: 'a,
    {
        let mut has_selected = false;
        let mut has_unselected = false;
        for id in ids {
            if self.selected.contains(id) {
                has_selected = true;
            } else {
                has_unselected = true;
            }
            if has_selected && has_unselected {
                return true;
            }
        }
        false
    }
}

/// Supported filter operators for headless table filtering.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFilterOperator {
    Equals,
    NotEquals,
    Contains,
    StartsWith,
    GreaterThan,
    LessThan,
    In,
}

/// Framework-agnostic filter descriptor for table queries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiFilterRule {
    pub field: String,
    pub operator: UiFilterOperator,
    pub value: String,
}

impl UiFilterRule {
    pub fn new(field: impl Into<String>, operator: UiFilterOperator, value: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            operator,
            value: value.into(),
        }
    }

    pub fn equals(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            operator: UiFilterOperator::Equals,
            value: value.into(),
        }
    }

    pub fn contains(field: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            field: field.into(),
            operator: UiFilterOperator::Contains,
            value: value.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_toggle_and_state_management() {
        let mut selection = UiSelectionState::new();
        assert!(selection.is_empty());
        assert_eq!(selection.count(), 0);

        assert!(selection.toggle("item-1".to_string()));
        assert!(selection.is_selected(&"item-1".to_string()));
        assert_eq!(selection.count(), 1);

        assert!(!selection.toggle("item-1".to_string()));
        assert!(!selection.is_selected(&"item-1".to_string()));
        assert_eq!(selection.count(), 0);
    }

    #[test]
    fn selection_all_and_partial_checks() {
        let mut selection = UiSelectionState::new();
        let items = vec!["a".to_string(), "b".to_string(), "c".to_string()];

        assert!(!selection.is_all_selected(&items));
        assert!(!selection.is_partially_selected(&items));

        selection.select("a".to_string());
        assert!(!selection.is_all_selected(&items));
        assert!(selection.is_partially_selected(&items));

        selection.select_all(items.clone());
        assert!(selection.is_all_selected(&items));
        assert!(!selection.is_partially_selected(&items));

        selection.deselect(&"b".to_string());
        assert!(selection.is_partially_selected(&items));
        assert!(!selection.is_all_selected(&items));

        selection.clear();
        assert!(selection.is_empty());
    }

    #[test]
    fn filter_rule_construction_and_serialization() {
        let rule = UiFilterRule::equals("status", "published");
        assert_eq!(rule.field, "status");
        assert_eq!(rule.operator, UiFilterOperator::Equals);
        assert_eq!(rule.value, "published");

        let json = serde_json::to_string(&rule).expect("filter rule should serialize");
        assert!(json.contains(r#""operator":"equals""#));

        let deserialized: UiFilterRule =
            serde_json::from_str(&json).expect("filter rule should deserialize");
        assert_eq!(deserialized, rule);
    }
}
