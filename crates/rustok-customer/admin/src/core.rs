pub const DEFAULT_CUSTOMER_PAGE: u64 = 1;
pub const DEFAULT_CUSTOMER_PER_PAGE: u64 = 24;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CustomerListRequest {
    pub search: String,
    pub page: u64,
    pub per_page: u64,
}

pub fn customer_list_request(search: impl Into<String>) -> CustomerListRequest {
    CustomerListRequest {
        search: search.into(),
        page: DEFAULT_CUSTOMER_PAGE,
        per_page: DEFAULT_CUSTOMER_PER_PAGE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn customer_list_request_uses_admin_defaults() {
        let request = customer_list_request("alice");

        assert_eq!(request.search, "alice");
        assert_eq!(request.page, DEFAULT_CUSTOMER_PAGE);
        assert_eq!(request.per_page, DEFAULT_CUSTOMER_PER_PAGE);
    }
}
