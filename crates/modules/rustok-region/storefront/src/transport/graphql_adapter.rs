use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

use crate::core::resolve_storefront_regions;
use crate::model::{StorefrontRegion, StorefrontRegionsData};

use super::ApiError;

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
    .map_err(|error| ApiError::Graphql(error.to_string()))
}

pub async fn fetch_regions(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ApiError> {
    fetch_storefront_regions_graphql(selected_region_id, locale).await
}

async fn fetch_storefront_regions_graphql(
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
