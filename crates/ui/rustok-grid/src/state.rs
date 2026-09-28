//! Composite grid state: the whole interaction model in one serializable value.

use serde::{Deserialize, Serialize};

use crate::{
    column::GridColumnDef,
    filter::{ColumnFilters, FilterValue},
    pagination::GridPagination,
    resize::ColumnWidths,
    selection::RowSelection,
    sort::{SortDirection, SortState},
};

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct GridState {
    pub column_widths: ColumnWidths,
    pub filters: ColumnFilters,
    pub sort: SortState,
    pub selection: RowSelection,
    pub pagination: GridPagination,
}

impl GridState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pagination(mut self, page: usize, page_size: usize, total: u64) -> Self {
        self.pagination = GridPagination::new(page, page_size, total);
        self
    }

    /// Repair every invariant at once — call this after deserializing state
    /// that came from a URL, local storage or an API.
    pub fn normalize(&mut self) {
        self.sort.normalize();
        self.pagination.normalize();
    }

    /// Apply a filter change. Any change to the result set invalidates the
    /// current page, so paging is reset to the first page.
    ///
    /// Returns `true` when the filter set actually changed.
    pub fn set_filter(&mut self, column_id: impl Into<String>, value: FilterValue) -> bool {
        let changed = self.filters.set(column_id, value);
        if changed {
            self.pagination.set_page(1);
        }
        changed
    }

    pub fn reset_filters(&mut self) -> bool {
        let changed = self.filters.clear();
        if changed {
            self.pagination.set_page(1);
        }
        changed
    }

    /// Toggle sorting on a column and reset paging (ordering changes which
    /// rows land on page 1).
    pub fn toggle_sort(&mut self, column_id: &str) -> Option<SortDirection> {
        self.sort.toggle(column_id);
        self.pagination.set_page(1);
        self.sort.is_sorted_by(column_id)
    }

    /// Drop selected rows that are no longer part of `visible_ids`.
    pub fn prune_selection(&mut self, visible_ids: impl IntoIterator<Item = impl Into<String>>) {
        self.selection.retain_ids(visible_ids);
    }

    /// Clamp persisted widths to the current column definitions.
    pub fn clamp_widths(&mut self, columns: &[GridColumnDef]) {
        for column in columns {
            let id = column.id.as_str();
            if let Some(width) = self.column_widths.get_stored(id) {
                self.column_widths
                    .set_clamped(id, width, column.width.min, column.width.max);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filter_changes_reset_pagination() {
        let mut state = GridState::new().with_pagination(3, 10, 100);
        assert!(state.set_filter("title", FilterValue::Text(" a ".into())));
        assert_eq!(state.pagination.page, 1);
        // Setting the same value again is a no-op.
        assert!(!state.set_filter("title", FilterValue::Text("a".into())));
        assert!(state.reset_filters());
        assert!(!state.reset_filters());
    }

    #[test]
    fn sorting_resets_pagination() {
        let mut state = GridState::new().with_pagination(4, 10, 100);
        assert_eq!(state.toggle_sort("title"), Some(SortDirection::Asc));
        assert_eq!(state.pagination.page, 1);
    }

    #[test]
    fn widths_are_clamped_against_current_columns() {
        let columns = vec![GridColumnDef::new("title", "Title").width_range(200, 120, 300)];
        let mut state = GridState::new();
        state.column_widths.set("title", 9_000);
        state.clamp_widths(&columns);
        assert_eq!(state.column_widths.get("title", 200), 300);
    }
}
