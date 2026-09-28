use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

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
}

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

impl FilterValue {
    pub fn is_empty(&self) -> bool {
        match self {
            Self::Empty => true,
            Self::Text(s) => s.trim().is_empty(),
            Self::Select(s) => s.trim().is_empty(),
            Self::NumberRange { min, max } => min.is_none() && max.is_none(),
            Self::DateRange { from, to } => from.is_none() && to.is_none(),
            Self::Boolean(_) => false,
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
}

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

    pub fn set(&mut self, column_id: impl Into<String>, value: FilterValue) {
        let key = column_id.into();
        if value.is_empty() {
            self.filters.remove(&key);
        } else {
            self.filters.insert(key, value);
        }
    }

    pub fn remove(&mut self, column_id: &str) {
        self.filters.remove(column_id);
    }

    pub fn clear(&mut self) {
        self.filters.clear();
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
}
