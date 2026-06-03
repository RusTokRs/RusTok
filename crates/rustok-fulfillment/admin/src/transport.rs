use crate::api;
pub use crate::api::ApiError;
use crate::model::{
    FulfillmentAdminBootstrap, ShippingOption, ShippingOptionDraft, ShippingOptionList,
    ShippingProfile,
};

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<FulfillmentAdminBootstrap, ApiError> {
    api::fetch_bootstrap(token, tenant_slug).await
}

pub async fn fetch_shipping_options(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    search: Option<String>,
    currency_code: Option<String>,
    provider_id: Option<String>,
    page: u64,
    per_page: u64,
) -> Result<ShippingOptionList, ApiError> {
    api::fetch_shipping_options(
        token,
        tenant_slug,
        tenant_id,
        search,
        currency_code,
        provider_id,
        page,
        per_page,
    )
    .await
}

pub async fn fetch_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<ShippingOption>, ApiError> {
    api::fetch_shipping_option(token, tenant_slug, tenant_id, id).await
}

pub async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    page: u64,
    per_page: u64,
) -> Result<Vec<ShippingProfile>, ApiError> {
    api::fetch_shipping_profiles(token, tenant_slug, tenant_id, page, per_page).await
}

pub async fn create_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    draft: ShippingOptionDraft,
) -> Result<ShippingOption, ApiError> {
    api::create_shipping_option(token, tenant_slug, tenant_id, draft).await
}

pub async fn update_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: ShippingOptionDraft,
) -> Result<ShippingOption, ApiError> {
    api::update_shipping_option(token, tenant_slug, tenant_id, id, draft).await
}

pub async fn deactivate_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingOption, ApiError> {
    api::deactivate_shipping_option(token, tenant_slug, tenant_id, id).await
}

pub async fn reactivate_shipping_option(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingOption, ApiError> {
    api::reactivate_shipping_option(token, tenant_slug, tenant_id, id).await
}
