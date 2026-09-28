//! Column filter specifications and the values users produce with them.
//!
//! [`ColumnFilters`] is the single source of truth handed to the data layer.
//! It only ever stores *normalized, non-empty* values, so a consumer can treat
//! "the key exists" as "this filter is active" without re-validating anything.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// One option of a `Select` filter.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterOption {
    pub value: String,
    pub label: String,
}

impl FilterOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
        }
    }
}

/// Kind of control a column exposes in the filter row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum GridFilterType {
    Text {
        placeholder: Option<String>,
    },
    Select {
        options: Vec<FilterOption>,
        placeholder: Option<String>,
    },
    NumberRange {
        min_placeholder: Option<String>,
        max_placeholder: Option<String>,
        step: Option<f64>,
    },
    DateRange {
        from_placeholder: Option<String>,
        to_placeholder: Option<String>,
    },
    Boolean {
        true_label: String,
        false_label: String,
    },
}

impl GridFilterType {
    pub fn text() -> Self {
        Self::Text { placeholder: None }
    }

    pub fn text_with_placeholder(placeholder: impl Into<String>) -> Self {
        Self::Text {
            placeholder: Some(placeholder.into()),
        }
    }

    pub fn select(options: Vec<FilterOption>) -> Self {
        Self::Select {
            options,
            placeholder: None,
        }
    }

    pub fn select_with_placeholder(
        options: Vec<FilterOption>,
        placeholder: impl Into<String>,
    ) -> Self {
        Self::Select {
            options,
            placeholder: Some(placeholder.into()),
        }
    }

    pub fn number_range() -> Self {
        Self::NumberRange {
            min_placeholder: None,
            max_placeholder: None,
            step: None,
        }
    }

    pub fn date_range() -> Self {
        Self::DateRange {
            from_placeholder: None,
            to_placeholder: None,
        }
    }

    pub fn boolean(true_label: impl Into<String>, false_label: impl Into<String>) -> Self {
        Self::Boolean {
            true_label: true_label.into(),
            false_label: false_label.into(),
        }
    }

    /// The neutral value produced by this control when the user clears it.
    pub fn empty_value(&self) -> FilterValue {
        match self {
            Self::Text { .. } => FilterValue::Text(String::new()),
            Self::Select { .. } => FilterValue::Select(String::new()),
            Self::NumberRange { .. } => FilterValue::NumberRange {
                min: None,
                max: None,
            },
            Self::DateRange { .. } => FilterValue::DateRange {
                from: None,
                to: None,
            },
            Self::Boolean { .. } => FilterValue::Empty,
        }
    }

    /// `true` when `value` was produced by this kind of control.
    pub fn accepts(&self, value: &FilterValue) -> bool {
        matches!(
            (self, value),
            (Self::Text { .. }, FilterValue::Text(_))
                | (Self::Select { .. }, FilterValue::Select(_))
                | (Self::NumberRange { .. }, FilterValue::NumberRange { .. })
                | (Self::DateRange { .. }, FilterValue::DateRange { .. })
                | (Self::Boolean { .. }, FilterValue::Boolean(_))
                | (_, FilterValue::Empty)
        )
    }
}

/// A value entered in the filter row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum FilterValue {
    Text(String),
    Select(String),
    NumberRange {
        min: Option<f64>,
        max: Option<f64>,
    },
    DateRange {
        from: Option<String>,
        to: Option<String>,
    },
    Boolean(bool),
    Empty,
}

fn clean_opt_string(value: Option<String>) -> Option<String> {
    value
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

fn clean_number(value: Option<f64>) -> Option<f64> {
    value.filter(|n| n.is_finite())
}

impl FilterValue {
    /// `true` when the value carries no constraint at all.
    ///
    /// `Boolean(false)` is *not* empty: filtering on "false" is a real query.
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Text(s) | Self::Select(s) => s.trim().is_empty(),
            Self::NumberRange { min, max } => {
                clean_number(*min).is_none() && clean_number(*max).is_none()
            }
            Self::DateRange { from, to } => {
                from.as_deref().map_or(true, |s| s.trim().is_empty())
                    && to.as_deref().map_or(true, |s| s.trim().is_empty())
            }
            Self::Boolean(_) => false,
        }
    }

    /// Canonical form of the value: trimmed strings, non-finite numbers
    /// dropped, and reversed ranges swapped into a meaningful order.
    ///
    /// Storing normalized values means two semantically identical filter sets
    /// compare equal, which keeps change-detection and caching honest.
    pub fn normalized(self) -> Self {
        match self {
            Self::Text(s) => Self::Text(s.trim().to_string()),
            Self::Select(s) => Self::Select(s.trim().to_string()),
            Self::NumberRange { min, max } => {
                let (mut min, mut max) = (clean_number(min), clean_number(max));
                if let (Some(lo), Some(hi)) = (min, max) {
                    if lo > hi {
                        min = Some(hi);
                        max = Some(lo);
                    }
                }
                Self::NumberRange { min, max }
            }
            Self::DateRange { from, to } => {
                let (mut from, mut to) = (clean_opt_string(from), clean_opt_string(to));
                // Dates are ISO-8601 (`YYYY-MM-DD`) so lexicographic ordering
                // matches chronological ordering.
                if let (Some(lo), Some(hi)) = (from.as_deref(), to.as_deref()) {
                    if lo > hi {
                        let swapped = (hi.to_string(), lo.to_string());
                        from = Some(swapped.0);
                        to = Some(swapped.1);
                    }
                }
                Self::DateRange { from, to }
            }
            other => other,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Text(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_select(&self) -> Option<&str> {
        match self {
            Self::Select(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn as_number_range(&self) -> Option<(Option<f64>, Option<f64>)> {
        match self {
            Self::NumberRange { min, max } => Some((*min, *max)),
            _ => None,
        }
    }

    pub fn as_date_range(&self) -> Option<(Option<&str>, Option<&str>)> {
        match self {
            Self::DateRange { from, to } => Some((from.as_deref(), to.as_deref())),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Boolean(b) => Some(*b),
            _ => None,
        }
    }
}

/// Active filters keyed by column id. Only non-empty values are kept.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ColumnFilters {
    filters: BTreeMap<String, FilterValue>,
}

impl ColumnFilters {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, column_id: &str) -> Option<&FilterValue> {
        self.filters.get(column_id)
    }

    pub fn contains(&self, column_id: &str) -> bool {
        self.filters.contains_key(column_id)
    }

    /// Store a filter value. Empty values remove the entry instead of
    /// persisting a no-op constraint; non-empty values are normalized first.
    ///
    /// Returns `true` when the stored state actually changed, which lets
    /// adapters skip needless refetches.
    pub fn set(&mut self, column_id: impl Into<String>, value: FilterValue) -> bool {
        let key = column_id.into();
        let value = value.normalized();
        if value.is_empty() {
            self.filters.remove(&key).is_some()
        } else {
            match self.filters.get(&key) {
                Some(existing) if *existing == value => false,
                _ => {
                    self.filters.insert(key, value);
                    true
                }
            }
        }
    }

    pub fn remove(&mut self, column_id: &str) -> bool {
        self.filters.remove(column_id).is_some()
    }

    pub fn clear(&mut self) -> bool {
        let had_any = !self.filters.is_empty();
        self.filters.clear();
        had_any
    }

    pub fn is_empty(&self) -> bool {
        self.filters.is_empty()
    }

    pub fn count(&self) -> usize {
        self.filters.len()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&String, &FilterValue)> {
        self.filters.iter()
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.filters.keys()
    }
}

impl<'a> IntoIterator for &'a ColumnFilters {
    type Item = (&'a String, &'a FilterValue);
    type IntoIter = std::collections::btree_map::Iter<'a, String, FilterValue>;

    fn into_iter(self) -> Self::IntoIter {
        self.filters.iter()
    }
}

impl FromIterator<(String, FilterValue)> for ColumnFilters {
    fn from_iter<I: IntoIterator<Item = (String, FilterValue)>>(iter: I) -> Self {
        let mut filters = Self::new();
        for (key, value) in iter {
            filters.set(key, value);
        }
        filters
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blank_ranges_are_not_persisted() {
        assert!(
            FilterValue::DateRange {
                from: Some(" ".into()),
                to: Some(String::new())
            }
            .is_empty()
        );
        assert!(
            FilterValue::NumberRange {
                min: Some(f64::NAN),
                max: None
            }
            .is_empty()
        );
        assert!(
            FilterValue::NumberRange {
                min: Some(f64::INFINITY),
                max: None
            }
            .is_empty()
        );
    }

    #[test]
    fn boolean_false_is_an_active_filter() {
        assert!(!FilterValue::Boolean(false).is_empty());
    }

    #[test]
    fn values_are_normalized_before_being_stored() {
        let mut filters = ColumnFilters::new();
        assert!(filters.set("title", FilterValue::Text("  hello  ".into())));
        assert_eq!(filters.get("title"), Some(&FilterValue::Text("hello".into())));

        // Same semantic value -> no change reported, no duplicate entry.
        assert!(!filters.set("title", FilterValue::Text("hello".into())));

        assert!(filters.set(
            "price",
            FilterValue::NumberRange {
                min: Some(100.0),
                max: Some(10.0)
            }
        ));
        assert_eq!(
            filters.get("price"),
            Some(&FilterValue::NumberRange {
                min: Some(10.0),
                max: Some(100.0)
            })
        );

        assert!(filters.set(
            "created_at",
            FilterValue::DateRange {
                from: Some("2026-05-01".into()),
                to: Some("2026-01-01".into())
            }
        ));
        assert_eq!(
            filters.get("created_at"),
            Some(&FilterValue::DateRange {
                from: Some("2026-01-01".into()),
                to: Some("2026-05-01".into())
            })
        );
    }

    #[test]
    fn clearing_an_input_removes_the_filter() {
        let mut filters = ColumnFilters::new();
        filters.set("status", FilterValue::Select("ACTIVE".into()));
        assert_eq!(filters.count(), 1);
        assert!(filters.set("status", FilterValue::Select(String::new())));
        assert!(filters.is_empty());
        assert!(!filters.clear());
    }

    #[test]
    fn filter_type_matches_its_value() {
        let ft = GridFilterType::text();
        assert!(ft.accepts(&FilterValue::Text("x".into())));
        assert!(ft.accepts(&FilterValue::Empty));
        assert!(!ft.accepts(&FilterValue::Boolean(true)));
        assert!(ft.empty_value().is_empty());
    }
}
