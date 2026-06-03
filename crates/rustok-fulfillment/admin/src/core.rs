pub const DEFAULT_SHIPPING_OPTION_PAGE: u64 = 1;
pub const DEFAULT_SHIPPING_OPTION_PER_PAGE: u64 = 24;
pub const DEFAULT_SHIPPING_PROFILE_PAGE: u64 = 1;
pub const DEFAULT_SHIPPING_PROFILE_PER_PAGE: u64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShippingOptionListRequest {
    pub search: Option<String>,
    pub currency_code: Option<String>,
    pub provider_id: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShippingProfileListRequest {
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

pub fn shipping_option_list_request(
    search: impl AsRef<str>,
    currency_code: impl AsRef<str>,
    provider_id: impl AsRef<str>,
) -> ShippingOptionListRequest {
    ShippingOptionListRequest {
        search: text_or_none(search),
        currency_code: text_or_none(currency_code),
        provider_id: text_or_none(provider_id),
        page: DEFAULT_SHIPPING_OPTION_PAGE,
        per_page: DEFAULT_SHIPPING_OPTION_PER_PAGE,
    }
}

pub fn shipping_profile_list_request() -> ShippingProfileListRequest {
    ShippingProfileListRequest {
        page: DEFAULT_SHIPPING_PROFILE_PAGE,
        per_page: DEFAULT_SHIPPING_PROFILE_PER_PAGE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shipping_option_request_trims_filters_and_uses_defaults() {
        let request = shipping_option_list_request(" express ", " USD ", " manual ");

        assert_eq!(request.search.as_deref(), Some("express"));
        assert_eq!(request.currency_code.as_deref(), Some("USD"));
        assert_eq!(request.provider_id.as_deref(), Some("manual"));
        assert_eq!(request.page, DEFAULT_SHIPPING_OPTION_PAGE);
        assert_eq!(request.per_page, DEFAULT_SHIPPING_OPTION_PER_PAGE);
    }

    #[test]
    fn blank_filter_normalizes_to_none() {
        assert_eq!(text_or_none("  "), None);
    }

    #[test]
    fn shipping_profile_request_uses_registry_defaults() {
        let request = shipping_profile_list_request();

        assert_eq!(request.page, DEFAULT_SHIPPING_PROFILE_PAGE);
        assert_eq!(request.per_page, DEFAULT_SHIPPING_PROFILE_PER_PAGE);
    }
}
