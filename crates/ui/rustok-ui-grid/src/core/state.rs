use serde::{Deserialize, Serialize};

use super::{
    filter::ColumnFilters, pagination::GridPagination, resize::ColumnWidths,
    selection::RowSelection, sort::SortState,
};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
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

    pub fn reset_filters(&mut self) {
        self.filters.clear();
        self.pagination.set_page(1);
    }
}
