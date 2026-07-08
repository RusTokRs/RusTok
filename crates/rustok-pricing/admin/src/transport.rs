mod graphql_adapter;
mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;

use crate::model::{
    PricingAdjustmentPreview, PricingAdminBootstrap, PricingDiscountDraft, PricingPriceDraft,
    PricingPriceListOption, PricingPriceListRuleDraft, PricingPriceListScopeDraft,
    PricingProductDetail, PricingProductList,
};
use native_server_adapter::ApiError;

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PricingAdminBootstrap, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_bootstrap(token, tenant_slug).await
        }
        UiTransportPath::Graphql => graphql_adapter::fetch_bootstrap(token, tenant_slug).await,
    }
}

pub async fn fetch_active_price_lists(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_active_price_lists(
                token,
                tenant_slug,
                channel_id,
                channel_slug,
            )
            .await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_active_price_lists(
                token,
                tenant_slug,
                graphql_adapter::parse_channel_id(channel_id)?,
                graphql_adapter::sanitize_channel_slug(channel_slug),
            )
            .await
        }
    }
}

pub async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<PricingProductList, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_products(
                token,
                tenant_slug,
                tenant_id,
                locale,
                search,
                status,
            )
            .await
        }
        UiTransportPath::Graphql => {
            graphql_adapter::fetch_products(
                token,
                tenant_slug,
                tenant_id,
                locale.unwrap_or_default(),
                search,
                status,
            )
            .await
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingProductDetail>, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => {
            native_server_adapter::fetch_product(
                token,
                tenant_slug,
                tenant_id,
                id,
                locale,
                currency_code,
                region_id,
                price_list_id,
                channel_id,
                channel_slug,
                quantity,
            )
            .await
        }
        UiTransportPath::Graphql => {
            let resolution_context = crate::core::sanitize_resolution_context(
                currency_code,
                region_id,
                price_list_id,
                quantity,
            )
            .map_err(ApiError::from)?;
            graphql_adapter::fetch_product(
                token,
                tenant_slug,
                tenant_id,
                id,
                locale.unwrap_or_default(),
                resolution_context
                    .as_ref()
                    .map(|context| context.currency_code.clone()),
                resolution_context
                    .as_ref()
                    .and_then(|context| context.region_id.clone()),
                resolution_context
                    .as_ref()
                    .and_then(|context| context.price_list_id.clone()),
                graphql_adapter::parse_channel_id(channel_id)?,
                graphql_adapter::sanitize_channel_slug(channel_slug),
                resolution_context.as_ref().map(|context| context.quantity),
            )
            .await
        }
    }
}

pub async fn update_variant_price(
    variant_id: String,
    payload: PricingPriceDraft,
) -> Result<(), ApiError> {
    native_server_adapter::update_variant_price(variant_id, payload).await
}

pub async fn preview_variant_discount(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ApiError> {
    native_server_adapter::preview_variant_discount(variant_id, payload).await
}

pub async fn apply_variant_discount(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ApiError> {
    native_server_adapter::apply_variant_discount(variant_id, payload).await
}

pub async fn update_price_list_rule(
    price_list_id: String,
    payload: PricingPriceListRuleDraft,
) -> Result<PricingPriceListOption, ApiError> {
    native_server_adapter::update_price_list_rule(price_list_id, payload).await
}

pub async fn update_price_list_scope(
    price_list_id: String,
    payload: PricingPriceListScopeDraft,
) -> Result<PricingPriceListOption, ApiError> {
    native_server_adapter::update_price_list_scope(price_list_id, payload).await
}
