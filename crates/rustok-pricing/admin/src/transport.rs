use crate::api::{self, ApiError};
use crate::model::{
    PricingAdjustmentPreview, PricingAdminBootstrap, PricingDiscountDraft, PricingPriceDraft,
    PricingPriceListOption, PricingPriceListRuleDraft, PricingPriceListScopeDraft,
    PricingProductDetail, PricingProductList,
};

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PricingAdminBootstrap, ApiError> {
    api::fetch_bootstrap(token, tenant_slug).await
}

pub async fn fetch_active_price_lists(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ApiError> {
    api::fetch_active_price_lists(token, tenant_slug, channel_id, channel_slug).await
}

pub async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<PricingProductList, ApiError> {
    api::fetch_products(token, tenant_slug, tenant_id, locale, search, status).await
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
    api::fetch_product(
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

pub async fn update_variant_price(
    variant_id: String,
    payload: PricingPriceDraft,
) -> Result<(), ApiError> {
    api::update_variant_price(variant_id, payload).await
}

pub async fn preview_variant_discount(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ApiError> {
    api::preview_variant_discount(variant_id, payload).await
}

pub async fn apply_variant_discount(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ApiError> {
    api::apply_variant_discount(variant_id, payload).await
}

pub async fn update_price_list_rule(
    price_list_id: String,
    payload: PricingPriceListRuleDraft,
) -> Result<PricingPriceListOption, ApiError> {
    api::update_price_list_rule(price_list_id, payload).await
}

pub async fn update_price_list_scope(
    price_list_id: String,
    payload: PricingPriceListScopeDraft,
) -> Result<PricingPriceListOption, ApiError> {
    api::update_price_list_scope(price_list_id, payload).await
}
