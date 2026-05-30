use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::core::resolve_storefront_regions;
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

const STOREFRONT_REGIONS_QUERY: &str = "query StorefrontRegions($locale: String) { storefrontRegions(locale: $locale) { id name currencyCode taxProviderId taxRate taxIncluded countryTaxPolicies { countryCode taxRate taxIncluded } countries } }";

#[derive(Debug, Deserialize)]
struct StorefrontRegionsResponse {
    #[serde(rename = "storefrontRegions")]
    storefront_regions: Vec<StorefrontRegion>,
}

#[derive(Debug, Serialize)]
struct StorefrontRegionsVariables {
    locale: Option<String>,
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

#[allow(dead_code)]
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

#[allow(dead_code)]
fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional(requested)
        .or_else(|| {
            request_context_locale.and_then(|value| normalize_optional(Some(value.to_string())))
        })
        .or_else(|| normalize_optional(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
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

async fn request<V, T>(query: &str, variables: Option<V>) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(ApiError::from)
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
    let response: StorefrontRegionsResponse = request(
        STOREFRONT_REGIONS_QUERY,
        Some(StorefrontRegionsVariables { locale }),
    )
    .await?;
    Ok(resolve_storefront_regions(
        response.storefront_regions,
        selected_region_id,
    ))
}

#[cfg(feature = "ssr")]
fn map_region(value: rustok_region::RegionResponse) -> StorefrontRegion {
    StorefrontRegion {
        id: value.id.to_string(),
        name: value.name,
        currency_code: value.currency_code,
        tax_provider_id: value.tax_provider_id,
        tax_rate: value.tax_rate.normalize().to_string(),
        tax_included: value.tax_included,
        country_tax_policies: value
            .country_tax_policies
            .into_iter()
            .map(|policy| crate::model::StorefrontRegionCountryTaxPolicy {
                country_code: policy.country_code,
                tax_rate: policy.tax_rate.normalize().to_string(),
                tax_included: policy.tax_included,
            })
            .collect(),
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
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let requested_locale = resolve_requested_locale(
            locale,
            request_context
                .as_ref()
                .map(|context| context.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let regions = RegionService::new(app_ctx.db.clone())
            .list_regions(
                tenant.id,
                Some(requested_locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_region)
            .collect();

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
