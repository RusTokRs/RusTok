//! Single-column sort state machine.
//!
//! The actual ordering is intentionally *not* performed here: server-driven
//! grids forward this state to the backend, client-side grids can apply it
//! with their own comparator.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

impl SortDirection {
    pub fn opposite(self) -> Self {
        match self {
            Self::Asc => Self::Desc,
            Self::Desc => Self::Asc,
        }
    }

    /// Value for the WAI-ARIA `aria-sort` attribute.
    pub fn aria_sort(self) -> &'static str {
        match self {
            Self::Asc => "ascending",
            Self::Desc => "descending",
        }
    }

    /// Lowercase keyword commonly accepted by REST/GraphQL APIs.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Asc => "asc",
            Self::Desc => "desc",
        }
    }
}

impl std::fmt::Display for SortDirection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Which column the grid is sorted by, if any.
///
/// The two fields are kept in sync: either both are `Some` or both are `None`.
/// Deserialized or hand-built values are repaired by [`SortState::normalize`].
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortState {
    pub column_id: Option<String>,
    pub direction: Option<SortDirection>,
}

impl SortState {
    pub fn new(column_id: impl Into<String>, direction: SortDirection) -> Self {
        Self {
            column_id: Some(column_id.into()),
            direction: Some(direction),
        }
    }

    /// Repair a half-filled state (e.g. coming from a URL query or storage).
    pub fn normalize(&mut self) {
        match (self.column_id.as_deref(), self.direction) {
            (Some(id), None) if !id.trim().is_empty() => {
                self.direction = Some(SortDirection::Asc);
            }
            (Some(id), _) if id.trim().is_empty() => self.clear(),
            (None, Some(_)) => self.direction = None,
            _ => {}
        }
    }

    pub fn is_active(&self) -> bool {
        self.column_id.is_some() && self.direction.is_some()
    }

    /// Active sort as a pair, `None` when the grid is unsorted.
    pub fn active(&self) -> Option<(&str, SortDirection)> {
        match (self.column_id.as_deref(), self.direction) {
            (Some(id), Some(direction)) => Some((id, direction)),
            _ => None,
        }
    }

    pub fn is_sorted_by(&self, column_id: &str) -> Option<SortDirection> {
        match self.active() {
            Some((id, direction)) if id == column_id => Some(direction),
            _ => None,
        }
    }

    /// `aria-sort` value for a header cell of `column_id`.
    pub fn aria_sort_for(&self, column_id: &str) -> &'static str {
        match self.is_sorted_by(column_id) {
            Some(direction) => direction.aria_sort(),
            None => "none",
        }
    }

    /// Cycles through: None -> Asc -> Desc -> None.
    pub fn toggle(&mut self, column_id: &str) {
        match self.is_sorted_by(column_id) {
            Some(SortDirection::Asc) => self.direction = Some(SortDirection::Desc),
            Some(SortDirection::Desc) => self.clear(),
            None => {
                self.column_id = Some(column_id.to_string());
                self.direction = Some(SortDirection::Asc);
            }
        }
    }

    pub fn set(&mut self, column_id: impl Into<String>, direction: SortDirection) {
        self.column_id = Some(column_id.into());
        self.direction = Some(direction);
    }

    pub fn clear(&mut self) {
        self.column_id = None;
        self.direction = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn toggle_cycles_asc_desc_none() {
        let mut state = SortState::default();
        state.toggle("title");
        assert_eq!(state.is_sorted_by("title"), Some(SortDirection::Asc));
        state.toggle("title");
        assert_eq!(state.is_sorted_by("title"), Some(SortDirection::Desc));
        state.toggle("title");
        assert!(!state.is_active());
        assert_eq!(state.aria_sort_for("title"), "none");
    }

    #[test]
    fn switching_column_restarts_at_asc() {
        let mut state = SortState::new("title", SortDirection::Desc);
        state.toggle("sku");
        assert_eq!(state.active(), Some(("sku", SortDirection::Asc)));
        assert_eq!(state.is_sorted_by("title"), None);
    }

    #[test]
    fn half_filled_state_is_repaired() {
        let mut state = SortState {
            column_id: Some("title".into()),
            direction: None,
        };
        state.normalize();
        assert_eq!(state.active(), Some(("title", SortDirection::Asc)));

        let mut dangling = SortState {
            column_id: None,
            direction: Some(SortDirection::Desc),
        };
        dangling.normalize();
        assert!(!dangling.is_active());

        let mut blank = SortState {
            column_id: Some("  ".into()),
            direction: Some(SortDirection::Asc),
        };
        blank.normalize();
        assert!(!blank.is_active());
    }
}
