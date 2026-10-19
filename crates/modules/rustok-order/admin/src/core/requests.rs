use rustok_ui_core::normalize_ui_text;

pub const DEFAULT_ORDER_PAGE: u64 = 1;
pub const DEFAULT_ORDER_PER_PAGE: u64 = 24;
pub const MAX_ORDER_PER_PAGE: u64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderListRequest {
    pub status: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

impl OrderListRequest {
    pub fn new(status: impl AsRef<str>, page: u64, per_page: u64) -> Self {
        let normalized_page = if page == 0 { DEFAULT_ORDER_PAGE } else { page };
        let normalized_per_page = match per_page {
            0 => DEFAULT_ORDER_PER_PAGE,
            p if p > MAX_ORDER_PER_PAGE => MAX_ORDER_PER_PAGE,
            p => p,
        };

        Self {
            status: text_or_none(status).map(|s| s.to_ascii_lowercase()),
            page: normalized_page,
            per_page: normalized_per_page,
        }
    }
}

pub fn text_or_none(value: impl AsRef<str>) -> Option<String> {
    normalize_ui_text(value.as_ref())
}

pub fn order_list_request(status: impl AsRef<str>) -> OrderListRequest {
    OrderListRequest::new(status, DEFAULT_ORDER_PAGE, DEFAULT_ORDER_PER_PAGE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_list_request_trims_status_and_uses_defaults() {
        let request = order_list_request(" paid ");

        assert_eq!(request.status.as_deref(), Some("paid"));
        assert_eq!(request.page, DEFAULT_ORDER_PAGE);
        assert_eq!(request.per_page, DEFAULT_ORDER_PER_PAGE);
    }

    #[test]
    fn order_list_request_lowercases_status() {
        let request = order_list_request(" CONFIRMED ");
        assert_eq!(request.status.as_deref(), Some("confirmed"));
    }

    #[test]
    fn order_list_request_bounds_pagination() {
        let zero_request = OrderListRequest::new("paid", 0, 0);
        assert_eq!(zero_request.page, DEFAULT_ORDER_PAGE);
        assert_eq!(zero_request.per_page, DEFAULT_ORDER_PER_PAGE);

        let capped_request = OrderListRequest::new("paid", 5, 200);
        assert_eq!(capped_request.page, 5);
        assert_eq!(capped_request.per_page, MAX_ORDER_PER_PAGE);
    }

    #[test]
    fn blank_text_normalizes_to_none() {
        assert_eq!(text_or_none("  "), None);
    }
}
