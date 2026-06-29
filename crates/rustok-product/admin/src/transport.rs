mod graphql_adapter;

use crate::model::{
    ProductAdminBootstrap, ProductDetail, ProductDraft, ProductList, ProductPricingDetail,
    ShippingProfileList,
};
use graphql_adapter::ApiError;

pub(crate) async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<ProductAdminBootstrap, ApiError> {
    graphql_adapter::fetch_bootstrap(token, tenant_slug).await
}

pub(crate) async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<ProductList, ApiError> {
    graphql_adapter::fetch_products(token, tenant_slug, tenant_id, locale, search, status).await
}

pub(crate) async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
) -> Result<Option<ProductDetail>, ApiError> {
    graphql_adapter::fetch_product(token, tenant_slug, tenant_id, id, locale).await
}

pub(crate) async fn fetch_product_pricing(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
    currency_code: Option<String>,
) -> Result<Option<ProductPricingDetail>, ApiError> {
    graphql_adapter::fetch_product_pricing(token, tenant_slug, tenant_id, id, locale, currency_code)
        .await
}

pub(crate) async fn fetch_shipping_profiles(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
) -> Result<ShippingProfileList, ApiError> {
    graphql_adapter::fetch_shipping_profiles(token, tenant_slug, tenant_id).await
}

pub(crate) async fn create_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, ApiError> {
    graphql_adapter::create_product(token, tenant_slug, tenant_id, user_id, draft).await
}

pub(crate) async fn update_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    draft: ProductDraft,
) -> Result<ProductDetail, ApiError> {
    graphql_adapter::update_product(token, tenant_slug, tenant_id, user_id, id, draft).await
}

pub(crate) async fn change_product_status(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
    status: &str,
) -> Result<ProductDetail, ApiError> {
    graphql_adapter::change_product_status(token, tenant_slug, tenant_id, user_id, id, status).await
}

pub(crate) async fn delete_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    user_id: String,
    id: String,
) -> Result<bool, ApiError> {
    graphql_adapter::delete_product(token, tenant_slug, tenant_id, user_id, id).await
}
