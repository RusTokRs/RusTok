use crate::api::{self, ApiError};
use crate::model::{
    CommerceAdminBootstrap, CommerceAdminCartSnapshot, CommerceCartPromotionDraft,
    CommerceCartPromotionPreview, CommerceOrderChange, CommerceOrderChangeActionDraft,
    CommerceOrderChangeList, ShippingProfile, ShippingProfileDraft, ShippingProfileList,
};

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<CommerceAdminBootstrap, ApiError> {
    api::fetch_bootstrap(token, tenant_slug).await
}

pub async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    search: Option<String>,
) -> Result<ShippingProfileList, ApiError> {
    api::fetch_shipping_profiles(token, tenant_slug, tenant_id, search).await
}

pub async fn fetch_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<Option<ShippingProfile>, ApiError> {
    api::fetch_shipping_profile(token, tenant_slug, tenant_id, id).await
}

pub async fn create_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    api::create_shipping_profile(token, tenant_slug, tenant_id, draft).await
}

pub async fn update_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    draft: ShippingProfileDraft,
) -> Result<ShippingProfile, ApiError> {
    api::update_shipping_profile(token, tenant_slug, tenant_id, id, draft).await
}

pub async fn deactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    api::deactivate_shipping_profile(token, tenant_slug, tenant_id, id).await
}

pub async fn reactivate_shipping_profile(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
) -> Result<ShippingProfile, ApiError> {
    api::reactivate_shipping_profile(token, tenant_slug, tenant_id, id).await
}

pub async fn fetch_order_changes(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_id: Option<String>,
    status: Option<String>,
) -> Result<CommerceOrderChangeList, ApiError> {
    api::fetch_order_changes(token, tenant_slug, tenant_id, order_id, status).await
}

pub async fn apply_order_change(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_change_id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ApiError> {
    api::apply_order_change(token, tenant_slug, tenant_id, order_change_id, draft).await
}

pub async fn cancel_order_change(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    order_change_id: String,
    draft: CommerceOrderChangeActionDraft,
) -> Result<CommerceOrderChange, ApiError> {
    api::cancel_order_change(token, tenant_slug, tenant_id, order_change_id, draft).await
}

pub async fn preview_cart_promotion(
    cart_id: String,
    draft: CommerceCartPromotionDraft,
) -> Result<CommerceCartPromotionPreview, ApiError> {
    api::preview_cart_promotion(cart_id, draft).await
}

pub async fn apply_cart_promotion(
    cart_id: String,
    draft: CommerceCartPromotionDraft,
) -> Result<CommerceAdminCartSnapshot, ApiError> {
    api::apply_cart_promotion(cart_id, draft).await
}
