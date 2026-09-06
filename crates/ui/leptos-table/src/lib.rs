use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SortRule {
    pub field: String,
    pub direction: SortDirection,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FilterRule {
    pub field: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TableState {
    pub page: u32,
    pub page_size: u32,
    pub sort: Vec<SortRule>,
    pub filters: Vec<FilterRule>,
}

impl TableState {
    pub fn new(page: u32, page_size: u32) -> Self {
        Self {
            page,
            page_size,
            sort: Vec::new(),
            filters: Vec::new(),
        }
    }

    pub fn with_sort(mut self, sort: Vec<SortRule>) -> Self {
        self.sort = sort;
        self
    }

    pub fn with_filters(mut self, filters: Vec<FilterRule>) -> Self {
        self.filters = filters;
        self
    }
}
