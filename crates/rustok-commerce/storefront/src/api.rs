use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::StorefrontCommerceData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
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

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn fallback_storefront_commerce(locale: Option<String>) -> StorefrontCommerceData {
    let effective_locale = normalize_optional(locale).unwrap_or_else(|| "en".to_string());

    StorefrontCommerceData {
        effective_locale: effective_locale.clone(),
        tenant_slug: configured_tenant_slug(),
        tenant_default_locale: effective_locale,
        channel_slug: None,
        channel_resolution_source: None,
    }
}

pub async fn fetch_storefront_commerce(
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    match fetch_storefront_commerce_server(locale.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => Ok(fallback_storefront_commerce(locale)),
    }
}

pub async fn fetch_storefront_commerce_server(
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    storefront_commerce_native(locale)
        .await
        .map_err(ApiError::from)
}

#[server(prefix = "/api/fn", endpoint = "commerce/storefront-data")]
async fn storefront_commerce_native(
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        Ok(StorefrontCommerceData {
            effective_locale: normalize_optional(locale).unwrap_or(request_context.locale),
            tenant_slug: Some(tenant.slug),
            tenant_default_locale: tenant.default_locale,
            channel_slug: request_context.channel_slug,
            channel_resolution_source: request_context
                .channel_resolution_source
                .map(|source| source.as_str().to_string()),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = locale;
        Err(ServerFnError::new(
            "commerce/storefront-data requires the `ssr` feature",
        ))
    }
}
