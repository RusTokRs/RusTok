use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{StorefrontRegion, StorefrontRegionsData};

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

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

const STOREFRONT_REGIONS_QUERY: &str = "query StorefrontRegions { storefrontRegions { id name currencyCode taxRate taxIncluded countries } }";

#[derive(Debug, Deserialize)]
struct StorefrontRegionsResponse {
    #[serde(rename = "storefrontRegions")]
    storefront_regions: Vec<StorefrontRegion>,
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

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<T>(query: &str) -> Result<T, ApiError>
where
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, None::<()>),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_storefront_regions(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ApiError> {
    match fetch_storefront_regions_server(selected_region_id.clone(), locale.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => fetch_storefront_regions_graphql(selected_region_id, locale).await,
    }
}

pub async fn fetch_storefront_regions_server(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ApiError> {
    storefront_regions_native(selected_region_id, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_regions_graphql(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ApiError> {
    let _ = locale;
    let response: StorefrontRegionsResponse = request(STOREFRONT_REGIONS_QUERY).await?;
    Ok(resolve_storefront_regions(
        response.storefront_regions,
        selected_region_id,
    ))
}

fn resolve_storefront_regions(
    regions: Vec<StorefrontRegion>,
    selected_region_id: Option<String>,
) -> StorefrontRegionsData {
    let resolved_selected_region_id = selected_region_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| regions.first().map(|item| item.id.clone()));
    let selected_region = resolved_selected_region_id
        .as_ref()
        .and_then(|selected_id| regions.iter().find(|item| &item.id == selected_id))
        .cloned();

    StorefrontRegionsData {
        regions,
        selected_region,
        selected_region_id: resolved_selected_region_id,
    }
}

#[cfg(feature = "ssr")]
fn map_region(value: rustok_region::RegionResponse) -> StorefrontRegion {
    StorefrontRegion {
        id: value.id.to_string(),
        name: value.name,
        currency_code: value.currency_code,
        tax_rate: value.tax_rate.normalize().to_string(),
        tax_included: value.tax_included,
        countries: value.countries,
    }
}

#[server(prefix = "/api/fn", endpoint = "region/storefront-data")]
async fn storefront_regions_native(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_region::RegionService;

        let app_ctx = expect_context::<AppContext>();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let regions = RegionService::new(app_ctx.db.clone())
            .list_regions(tenant.id)
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_region)
            .collect();

        let _ = locale;
        Ok(resolve_storefront_regions(regions, selected_region_id))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_region_id, locale);
        Err(ServerFnError::new(
            "region/storefront-data requires the `ssr` feature",
        ))
    }
}
