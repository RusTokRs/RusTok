pub mod graphql_adapter;
pub mod native_server_adapter;

use crate::api::ApiError;
use crate::model::StorefrontRegionsData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionFetchFallbackPolicy {
    NativeThenGraphql,
}

pub const DEFAULT_FALLBACK_POLICY: RegionFetchFallbackPolicy =
    RegionFetchFallbackPolicy::NativeThenGraphql;

pub async fn fetch_regions(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, ApiError> {
    fetch_regions_with_policy(selected_region_id, locale, DEFAULT_FALLBACK_POLICY).await
}

pub async fn fetch_regions_with_policy(
    selected_region_id: Option<String>,
    locale: Option<String>,
    policy: RegionFetchFallbackPolicy,
) -> Result<StorefrontRegionsData, ApiError> {
    match policy {
        RegionFetchFallbackPolicy::NativeThenGraphql => {
            match native_server_adapter::fetch_regions(selected_region_id.clone(), locale.clone())
                .await
            {
                Ok(data) => Ok(data),
                Err(_) => graphql_adapter::fetch_regions(selected_region_id, locale).await,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_preserves_native_then_graphql_fallback_contract() {
        assert_eq!(
            DEFAULT_FALLBACK_POLICY,
            RegionFetchFallbackPolicy::NativeThenGraphql
        );
    }
}
