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

#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct GridState {
    pub column_widths: ColumnWidths,
    pub filters: ColumnFilters,
    pub sort: SortState,
    pub selection: RowSelection,
    pub pagination: GridPagination,
}

impl<'de> Deserialize<'de> for GridState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize, Default)]
        #[serde(default)]
        struct GridStateData {
            column_widths: ColumnWidths,
            filters: ColumnFilters,
            sort: SortState,
            selection: RowSelection,
            pagination: GridPagination,
        }

        let data = GridStateData::deserialize(deserializer)?;
        let mut state = Self {
            column_widths: data.column_widths,
            filters: data.filters,
            sort: data.sort,
            selection: data.selection,
            pagination: data.pagination,
        };
        state.normalize();
        Ok(state)
    }
}

impl GridState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_pagination(mut self, page: usize, page_size: usize, total: u64) -> Self {
        self.pagination = GridPagination::new(page, page_size, total);
        self
    }

    /// Repair every invariant at once after direct field mutation. Serde deserialization calls
    /// this automatically; width overrides still need column definitions to be clamped.
    pub fn normalize(&mut self) {
        // `ColumnWidths` stores integer overrides; column-specific bounds are repaired by
        // `clamp_widths`, once the current definitions are available.
        self.filters.normalize();
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

    /// Clamp persisted widths to the current column definitions and drop the overrides of columns
    /// that no longer exist.
    pub fn clamp_widths(&mut self, columns: &[GridColumnDef]) {
        for column in columns {
            let id = column.id.as_str();
            if let Some(width) = self.column_widths.get_stored(id) {
                self.column_widths
                    .set_clamped(id, width, column.width.min, column.width.max);
            }
        }
        self.column_widths
            .prune_to(columns.iter().map(|column| column.id.as_str()));
    }

    /// Turn a full data set into the rows this state renders: filter, then sort, then page.
    ///
    /// The order is part of the contract — paging before sorting would sort a single page, and
    /// sorting before filtering would sort rows that are about to be dropped — and it is the order
    /// the TypeScript twin's `applyGridRows` uses. The owner still supplies the only piece the grid
    /// cannot know: which cell of a row belongs to which column.
    pub fn apply_rows<T, F>(&self, items: &[T], cell_of: F) -> Vec<T>
    where
        T: Clone,
        F: Fn(&T, &str) -> crate::cell::CellValue,
    {
        let mut rows = self.filters.retain_matching(items, &cell_of);
        self.sort.sort_items(&mut rows, &cell_of);
        let (start, end) = self.pagination.slice_bounds(rows.len());
        rows[start..end].to_vec()
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

    #[test]
    fn apply_rows_filters_sorts_and_pages_in_that_order() {
        use crate::cell::CellValue;
        use crate::filter::FilterValue;

        #[derive(Clone, Debug, PartialEq)]
        struct Row(&'static str, f64);

        let rows: Vec<Row> = vec![
            Row("hat", 30.0),
            Row("shirt", 10.0),
            Row("coat", 50.0),
            Row("boots", 20.0),
            Row("dress", 40.0),
        ];
        let cell_of = |row: &Row, column_id: &str| match column_id {
            "name" => CellValue::text(row.0),
            "price" => CellValue::number(row.1),
            _ => CellValue::Empty,
        };

        let mut state = GridState::new().with_pagination(1, 2, 5);
        assert_eq!(
            state.apply_rows(&rows, cell_of),
            vec![Row("hat", 30.0), Row("shirt", 10.0)]
        );

        // Sorting happens over the filtered set, so page 2 holds the middle of the ordered rows.
        state.sort.set("price", crate::sort::SortDirection::Desc);
        state.pagination.set_page(2);
        assert_eq!(
            state.apply_rows(&rows, cell_of),
            vec![Row("hat", 30.0), Row("boots", 20.0)]
        );

        // Filtering narrows the pool the page indexes into; a page past the end renders empty.
        state.set_filter("name", FilterValue::Text("o".into()));
        state.pagination.set_page(1);
        assert_eq!(
            state.apply_rows(&rows, cell_of),
            vec![Row("coat", 50.0), Row("boots", 20.0)]
        );
        state.pagination.set_page(2);
        assert!(state.apply_rows(&rows, cell_of).is_empty());
    }

    #[test]
    fn stale_width_overrides_are_pruned_by_clamping() {
        let columns = vec![GridColumnDef::new("title", "Title").width_range(200, 120, 300)];
        let mut state = GridState::new();
        state.column_widths.set("title", 240);
        state.column_widths.set("retired", 800);
        state.clamp_widths(&columns);
        assert_eq!(state.column_widths.get_stored("title"), Some(240));
        assert_eq!(state.column_widths.get_stored("retired"), None);
    }

    #[test]
    fn deserialization_repairs_composite_state() {
        let state: GridState = serde_json::from_str(
            r#"{"column_widths":{"widths":{"title":9000}},"filters":{"filters":{"empty":{"Text":" "}}},"sort":{"column_id":"  ","direction":"Asc"},"selection":{"selected_ids":[]},"pagination":{"page":99,"page_size":0,"total":21,"has_next":true,"mode":"Paged"}}"#,
        )
        .expect("valid state JSON");
        assert!(state.filters.is_empty());
        assert!(!state.sort.is_active());
        assert_eq!(state.pagination.page_size, 1);
        assert_eq!(state.pagination.page, 21);
        assert!(!state.pagination.has_next);
        // This override needs column definitions before it can be clamped.
        assert_eq!(state.column_widths.get_stored("title"), Some(9_000));
    }

    #[test]
    fn missing_state_fields_use_normal_defaults() {
        let state: GridState = serde_json::from_str("{}").expect("empty state JSON");
        assert_eq!(state.pagination, GridPagination::default());
        assert!(state.filters.is_empty());
        assert!(!state.sort.is_active());
    }
}
