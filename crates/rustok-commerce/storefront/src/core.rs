use crate::i18n::t;
use crate::model::{StorefrontCheckoutCart, StorefrontCommerceData};

pub const SELECTED_CART_QUERY_KEY: &str = "cart_id";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommerceStorefrontRouteState {
    pub selected_cart_id: Option<String>,
    pub selected_cart_query_key: &'static str,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommerceStorefrontShellViewModel {
    pub badge: String,
    pub title: String,
    pub subtitle: String,
    pub load_error: String,
    pub action_error: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CommerceStorefrontContextViewModel {
    pub effective_locale: String,
    pub tenant: String,
    pub tenant_default_locale: String,
    pub channel: String,
    pub channel_resolution_source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FetchCommerceRequest {
    pub selected_cart_id: Option<String>,
    pub locale: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CartCommandRequest {
    pub cart_id: String,
}

#[derive(Clone, Debug)]
pub struct SelectShippingOptionRequest {
    pub cart: StorefrontCheckoutCart,
    pub shipping_profile_slug: String,
    pub seller_id: Option<String>,
    pub seller_scope: Option<String>,
    pub shipping_option_id: Option<String>,
}

pub fn build_storefront_route_state(
    selected_cart_id: Option<String>,
) -> CommerceStorefrontRouteState {
    CommerceStorefrontRouteState {
        selected_cart_id: normalize_optional(selected_cart_id),
        selected_cart_query_key: SELECTED_CART_QUERY_KEY,
    }
}

pub fn build_storefront_shell_view_model(locale: Option<&str>) -> CommerceStorefrontShellViewModel {
    CommerceStorefrontShellViewModel {
        badge: t(locale, "commerce.badge", "commerce"),
        title: t(locale, "commerce.title", "Commerce orchestration hub"),
        subtitle: t(
            locale,
            "commerce.subtitle",
            "Catalog, pricing, regions, and cart line-item handling now live in module-owned storefront packages. Commerce remains the aggregate storefront handoff for checkout context and cross-domain flow.",
        ),
        load_error: t(
            locale,
            "commerce.error.load",
            "Failed to load commerce storefront data",
        ),
        action_error: t(
            locale,
            "commerce.error.action",
            "Failed to update aggregate checkout state",
        ),
    }
}

pub fn build_storefront_context_view_model(
    data: StorefrontCommerceData,
    locale: Option<&str>,
) -> CommerceStorefrontContextViewModel {
    let empty_value = t(locale, "commerce.context.empty", "not resolved");

    CommerceStorefrontContextViewModel {
        effective_locale: data.effective_locale,
        tenant: data
            .tenant_slug
            .unwrap_or_else(|| t(locale, "commerce.context.tenantMissing", "host tenant")),
        tenant_default_locale: data.tenant_default_locale,
        channel: data.channel_slug.unwrap_or_else(|| empty_value.clone()),
        channel_resolution_source: data
            .channel_resolution_source
            .unwrap_or_else(|| empty_value.clone()),
    }
}

pub fn build_fetch_commerce_request(
    selected_cart_id: Option<String>,
    locale: Option<String>,
) -> FetchCommerceRequest {
    FetchCommerceRequest {
        selected_cart_id: normalize_optional(selected_cart_id),
        locale: normalize_optional(locale),
    }
}

pub fn build_cart_command_request(cart_id: String) -> CartCommandRequest {
    CartCommandRequest {
        cart_id: normalize_required(cart_id),
    }
}

pub fn build_select_shipping_option_request(
    cart: StorefrontCheckoutCart,
    shipping_profile_slug: String,
    seller_id: Option<String>,
    seller_scope: Option<String>,
    shipping_option_id: Option<String>,
) -> SelectShippingOptionRequest {
    SelectShippingOptionRequest {
        cart,
        shipping_profile_slug: normalize_required(shipping_profile_slug),
        seller_id: normalize_optional(seller_id),
        seller_scope: normalize_optional(seller_scope),
        shipping_option_id: normalize_optional(shipping_option_id),
    }
}

pub fn error_with_context(context: &str, error: &str) -> String {
    format!("{context}: {error}")
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

fn normalize_required(value: String) -> String {
    value.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_state_normalizes_blank_cart_id() {
        let state = build_storefront_route_state(Some("  ".to_string()));
        assert_eq!(state.selected_cart_id, None);
        assert_eq!(state.selected_cart_query_key, SELECTED_CART_QUERY_KEY);
    }

    #[test]
    fn route_state_trims_cart_id() {
        let state = build_storefront_route_state(Some(" cart-1 ".to_string()));
        assert_eq!(state.selected_cart_id.as_deref(), Some("cart-1"));
    }

    fn storefront_data(
        tenant_slug: Option<&str>,
        channel_slug: Option<&str>,
        channel_resolution_source: Option<&str>,
    ) -> StorefrontCommerceData {
        StorefrontCommerceData {
            effective_locale: "ru".to_string(),
            tenant_slug: tenant_slug.map(str::to_string),
            tenant_default_locale: "en".to_string(),
            channel_slug: channel_slug.map(str::to_string),
            channel_resolution_source: channel_resolution_source.map(str::to_string),
            selected_cart_id: None,
            checkout: None,
        }
    }

    #[test]
    fn context_view_model_preserves_resolved_context() {
        let view_model = build_storefront_context_view_model(
            storefront_data(Some("main"), Some("web"), Some("domain")),
            Some("en"),
        );

        assert_eq!(view_model.effective_locale, "ru");
        assert_eq!(view_model.tenant, "main");
        assert_eq!(view_model.tenant_default_locale, "en");
        assert_eq!(view_model.channel, "web");
        assert_eq!(view_model.channel_resolution_source, "domain");
    }

    #[test]
    fn context_view_model_applies_missing_context_fallbacks() {
        let view_model =
            build_storefront_context_view_model(storefront_data(None, None, None), Some("en"));

        assert_eq!(view_model.tenant, "host tenant");
        assert_eq!(view_model.channel, "not resolved");
        assert_eq!(view_model.channel_resolution_source, "not resolved");
    }

    #[test]
    fn fetch_request_normalizes_route_context_inputs() {
        let request = build_fetch_commerce_request(Some(" cart-1 ".into()), Some(" ru ".into()));
        assert_eq!(request.selected_cart_id.as_deref(), Some("cart-1"));
        assert_eq!(request.locale.as_deref(), Some("ru"));
    }

    #[test]
    fn cart_command_request_trims_command_id() {
        let request = build_cart_command_request(" cart-1 ".into());
        assert_eq!(request.cart_id, "cart-1");
    }
}
