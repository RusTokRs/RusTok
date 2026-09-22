use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, Default, ToSchema)]
pub struct PaginationParams {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

impl PaginationParams {
    pub fn offset(&self) -> u64 {
        (self.page.saturating_sub(1)) * self.per_page
    }

    pub fn limit(&self) -> u64 {
        self.per_page.min(100)
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginatedResponse<T> {
    pub data: Vec<T>,
    pub meta: PaginationMeta,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PaginationMeta {
    pub page: u64,
    pub per_page: u64,
    pub total: u64,
    pub total_pages: u64,
    pub has_next: bool,
    pub has_prev: bool,
}

impl PaginationMeta {
    pub fn new(page: u64, per_page: u64, total: u64) -> Self {
        let total_pages = total.div_ceil(per_page);
        Self {
            page,
            per_page,
            total,
            total_pages,
            has_next: page < total_pages,
            has_prev: page > 1,
        }
    }
}
