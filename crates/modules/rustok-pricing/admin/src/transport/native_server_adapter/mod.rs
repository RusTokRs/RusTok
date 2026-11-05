#![allow(clippy::too_many_arguments)]

use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::core::{
    PricingAdminRequestError, parse_optional_uuid_string, sanitize_channel_slug,
    sanitize_resolution_context,
};
use crate::model::{
    PricingAdjustmentPreview, PricingAdminBootstrap, PricingDiscountDraft, PricingPriceDraft,
    PricingPriceListOption, PricingPriceListRuleDraft, PricingPriceListScopeDraft,
    PricingProductDetail, PricingProductList,
};

#[cfg(feature = "ssr")]
pub(crate) mod context;
#[cfg(feature = "ssr")]
pub(crate) mod mappers;
#[cfg(feature = "ssr")]
pub(crate) mod operations;
pub(crate) mod server;

#[cfg(feature = "ssr")]
pub(crate) use context::*;
#[cfg(feature = "ssr")]
pub(crate) use mappers::*;
#[cfg(feature = "ssr")]
pub(crate) use operations::*;
pub(crate) use server::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

impl From<PricingAdminRequestError> for ApiError {
    fn from(value: PricingAdminRequestError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_bootstrap(
    _token: Option<String>,
    _tenant_slug: Option<String>,
) -> Result<PricingAdminBootstrap, ApiError> {
    pricing_admin_bootstrap_native()
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_active_price_lists(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ApiError> {
    let channel_id =
        parse_optional_uuid_string(channel_id, "channel_id").map_err(ApiError::from)?;
    let channel_slug = sanitize_channel_slug(channel_slug);
    pricing_admin_active_price_lists_native(channel_id, channel_slug)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_products(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    _tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<PricingProductList, ApiError> {
    pricing_admin_products_native(locale.unwrap_or_default(), search, status)
        .await
        .map_err(ApiError::from)
}

#[allow(clippy::too_many_arguments)]
pub async fn fetch_product(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    _tenant_id: String,
    id: String,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingProductDetail>, ApiError> {
    let channel_id =
        parse_optional_uuid_string(channel_id, "channel_id").map_err(ApiError::from)?;
    let channel_slug = sanitize_channel_slug(channel_slug);
    let resolution_context =
        sanitize_resolution_context(currency_code, region_id, price_list_id, quantity)
            .map_err(ApiError::from)?;
    let currency_code = resolution_context
        .as_ref()
        .map(|context| context.currency_code.clone());
    let region_id = resolution_context
        .as_ref()
        .and_then(|context| context.region_id.clone());
    let price_list_id = resolution_context
        .as_ref()
        .and_then(|context| context.price_list_id.clone());
    let quantity = resolution_context.as_ref().map(|context| context.quantity);
    pricing_admin_product_native(
        id,
        locale.unwrap_or_default(),
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn update_variant_price(
    variant_id: String,
    payload: PricingPriceDraft,
) -> Result<(), ApiError> {
    pricing_admin_update_variant_price_native(variant_id, payload)
        .await
        .map_err(Into::into)
}

pub async fn preview_variant_discount(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ApiError> {
    pricing_admin_preview_variant_discount_native(variant_id, payload)
        .await
        .map_err(Into::into)
}

pub async fn apply_variant_discount(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ApiError> {
    pricing_admin_apply_variant_discount_native(variant_id, payload)
        .await
        .map_err(Into::into)
}

pub async fn update_price_list_rule(
    price_list_id: String,
    payload: PricingPriceListRuleDraft,
) -> Result<PricingPriceListOption, ApiError> {
    pricing_admin_update_price_list_rule_native(price_list_id, payload)
        .await
        .map_err(Into::into)
}

pub async fn update_price_list_scope(
    price_list_id: String,
    payload: PricingPriceListScopeDraft,
) -> Result<PricingPriceListOption, ApiError> {
    pricing_admin_update_price_list_scope_native(price_list_id, payload)
        .await
        .map_err(Into::into)
}
