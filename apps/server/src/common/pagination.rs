use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Deserialize, ToSchema)]
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

impl Default for PaginationParams {
    fn default() -> Self {
        Self {
            page: default_page(),
            per_page: default_per_page(),
        }
    }
}

impl PaginationParams {
    /// The offset must use the same normalized page size as the query limit.
    /// Saturation avoids arithmetic overflow for hostile page numbers.
    pub fn offset(&self) -> u64 {
        self.page
            .max(1)
            .saturating_sub(1)
            .saturating_mul(self.limit())
    }

    pub fn limit(&self) -> u64 {
        self.per_page.clamp(1, 100)
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
        let page = page.max(1);
        let per_page = per_page.clamp(1, 100);
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_params_are_valid_and_consistent() {
        let params = PaginationParams::default();
        assert_eq!(params.page, 1);
        assert_eq!(params.per_page, 20);
        assert_eq!(params.offset(), 0);
        assert_eq!(params.limit(), 20);
    }

    #[test]
    fn offset_uses_the_same_bounded_page_size_as_limit() {
        let params = PaginationParams {
            page: 3,
            per_page: 1000,
        };
        assert_eq!(params.limit(), 100);
        assert_eq!(params.offset(), 200);
    }

    #[test]
    fn invalid_and_extreme_inputs_cannot_panic_or_wrap() {
        let params = PaginationParams {
            page: u64::MAX,
            per_page: u64::MAX,
        };
        assert_eq!(params.limit(), 100);
        assert_eq!(params.offset(), u64::MAX);

        let zero = PaginationParams {
            page: 0,
            per_page: 0,
        };
        assert_eq!(zero.offset(), 0);
        assert_eq!(zero.limit(), 1);
    }

    #[test]
    fn response_metadata_normalizes_zero_and_oversized_page_sizes() {
        let zero = PaginationMeta::new(0, 0, 5);
        assert_eq!((zero.page, zero.per_page, zero.total_pages), (1, 1, 5));
        assert!(zero.has_next);
        assert!(!zero.has_prev);

        let huge = PaginationMeta::new(2, u64::MAX, 250);
        assert_eq!((huge.page, huge.per_page, huge.total_pages), (2, 100, 3));
        assert!(huge.has_next);

        let empty = PaginationMeta::new(1, 0, 0);
        assert_eq!(empty.total_pages, 0);
        assert!(!empty.has_next);
    }
}
