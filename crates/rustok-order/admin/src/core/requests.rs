pub const DEFAULT_ORDER_PAGE: u64 = 1;
pub const DEFAULT_ORDER_PER_PAGE: u64 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderListRequest {
    pub status: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

pub fn text_or_none(value: impl AsRef<str>) -> Option<String> {
    let trimmed = value.as_ref().trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

pub fn order_list_request(status: impl AsRef<str>) -> OrderListRequest {
    OrderListRequest {
        status: text_or_none(status),
        page: DEFAULT_ORDER_PAGE,
        per_page: DEFAULT_ORDER_PER_PAGE,
    }
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
    fn blank_text_normalizes_to_none() {
        assert_eq!(text_or_none("  "), None);
    }
}
