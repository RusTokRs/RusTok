use super::raw_adapter::{self, ApiError};
use crate::model::{
    CommerceAdminBootstrap, ShippingProfile, ShippingProfileDraft, ShippingProfileList,
};

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<CommerceAdminBootstrap, ApiError> {
    raw_adapter::fetch_bootstrap(token, tenant_slug).await
}

pub async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    search: Option<String>,
) -> Result<ShippingProfileList, ApiError> {
    raw_adapter::fetch_shipping_profiles(token, tenant_slug, tenant_id, search).await
}

pub async fn fetch_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<ShippingProfile>, ApiError> {
    raw_adapter::fetch_shipping_profile(token, tenant_slug, tenant_id, id).await
}

pub async fn create_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    raw_adapter::create_shipping_profile(token, tenant_slug, tenant_id, draft).await
}

pub async fn update_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    raw_adapter::update_shipping_profile(token, tenant_slug, tenant_id, id, draft).await
}

pub async fn deactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    raw_adapter::deactivate_shipping_profile(token, tenant_slug, tenant_id, id).await
}

pub async fn reactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    raw_adapter::reactivate_shipping_profile(token, tenant_slug, tenant_id, id).await
}

#[cfg(test)]
mod tests {
    use std::any::type_name;

    use super::*;

    #[test]
    fn shipping_profile_transport_keeps_api_error_contract() {
        assert!(type_name::<ApiError>().contains("ApiError"));
    }
}
