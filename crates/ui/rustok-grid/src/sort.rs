//! Single-column sort state machine.
//!
//! Ordering rules live with the cell vocabulary in [`crate::cell`]: a cell knows how it compares,
//! and [`SortState::sort_items`] is the loop-shaped entry point a client-side grid uses. A
//! server-driven grid forwards this state to its backend instead.

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
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct SortState {
    pub column_id: Option<String>,
    pub direction: Option<SortDirection>,
}

impl<'de> Deserialize<'de> for SortState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct SortStateData {
            #[serde(default)]
            column_id: Option<String>,
            #[serde(default)]
            direction: Option<SortDirection>,
        }

        let data = SortStateData::deserialize(deserializer)?;
        let mut state = Self {
            column_id: data.column_id,
            direction: data.direction,
        };
        state.normalize();
        Ok(state)
    }
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
        self.active().is_some()
    }

    /// Active sort as a normalized pair, `None` when the grid is unsorted.
    ///
    /// A nonblank column with a missing direction uses ascending order, matching `normalize()`.
    pub fn active(&self) -> Option<(&str, SortDirection)> {
        match (self.column_id.as_deref(), self.direction) {
            (Some(id), direction) if !id.trim().is_empty() => {
                Some((id, direction.unwrap_or(SortDirection::Asc)))
            }
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
        self.normalize();
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
        self.normalize();
    }

    pub fn clear(&mut self) {
        self.column_id = None;
        self.direction = None;
    }

    /// Order `items` in place by the active sort column, stably.
    ///
    /// Equal cells keep their input order — `slice::sort_by` is stable — so a page refresh cannot
    /// reshuffle a column of duplicate values. An inactive (or half-filled, hence ascending) state
    /// leaves the slice untouched.
    pub fn sort_items<T, F>(&self, items: &mut [T], cell_of: F)
    where
        F: Fn(&T, &str) -> crate::cell::CellValue,
    {
        let Some((column_id, direction)) = self.active() else {
            return;
        };
        items.sort_by(|left, right| {
            let order = cell_of(left, column_id).compare(&cell_of(right, column_id));
            match direction {
                SortDirection::Asc => order,
                SortDirection::Desc => order.reverse(),
            }
        });
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
    fn deserialization_repairs_half_filled_sort_state() {
        let state: SortState = serde_json::from_str(r#"{"column_id":"title","direction":null}"#)
            .expect("valid sort JSON");
        assert_eq!(state.active(), Some(("title", SortDirection::Asc)));

        let dangling: SortState =
            serde_json::from_str(r#"{"direction":"Desc"}"#).expect("valid sort JSON");
        assert!(!dangling.is_active());
    }

    #[test]
    fn queries_read_half_filled_state_as_normalized() {
        let half_filled = SortState {
            column_id: Some("title".into()),
            direction: None,
        };
        assert!(half_filled.is_active());
        assert_eq!(half_filled.active(), Some(("title", SortDirection::Asc)));
        assert_eq!(half_filled.is_sorted_by("title"), Some(SortDirection::Asc));
        assert_eq!(half_filled.aria_sort_for("title"), "ascending");

        let blank = SortState {
            column_id: Some("  ".into()),
            direction: Some(SortDirection::Desc),
        };
        assert!(!blank.is_active());
        assert_eq!(blank.active(), None);
    }

    #[test]
    fn mutations_normalize_invalid_existing_or_requested_sort_state() {
        let mut half_filled = SortState {
            column_id: Some("title".into()),
            direction: None,
        };
        half_filled.toggle("title");
        assert_eq!(half_filled.active(), Some(("title", SortDirection::Desc)));

        half_filled.set("  ", SortDirection::Asc);
        assert!(!half_filled.is_active());
    }

    #[test]
    fn sort_items_orders_by_the_active_column_and_leaves_ties_in_place() {
        #[derive(Clone, Debug, PartialEq)]
        struct Row(&'static str, i64);

        let rows = &mut [Row("c", 2), Row("a", 3), Row("b", 1), Row("d", 2)];
        let cell_of = |row: &Row, column_id: &str| match column_id {
            "name" => crate::cell::CellValue::text(row.0),
            "qty" => crate::cell::CellValue::number(row.1 as f64),
            _ => crate::cell::CellValue::Empty,
        };

        SortState::new("name", SortDirection::Asc).sort_items(rows, cell_of);
        assert_eq!(*rows, [Row("a", 3), Row("b", 1), Row("c", 2), Row("d", 2)]);

        // Descending reverses the comparison, not the tie order: qty 2 keeps c before d.
        SortState::new("qty", SortDirection::Desc).sort_items(rows, cell_of);
        assert_eq!(*rows, [Row("a", 3), Row("c", 2), Row("d", 2), Row("b", 1)]);

        // A half-filled state sorts ascending, and an inactive state is a no-op.
        let half = SortState {
            column_id: Some("qty".into()),
            direction: None,
        };
        half.sort_items(rows, cell_of);
        assert_eq!(rows[0].1, 1);
        SortState::default().sort_items(rows, cell_of);
        assert_eq!(rows[0].1, 1);
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
