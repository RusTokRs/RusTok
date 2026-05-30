pub(crate) mod graphql_adapter;
pub(crate) mod native_server_adapter;

use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

use crate::api::ApiError;
use crate::core::{RegionErrorEvidence, RegionStorefrontErrorPath};
use crate::model::StorefrontRegionsData;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionFetchFallbackPolicy {
    NativeThenGraphql,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
pub enum RegionTransportPath {
    NativeServer,
    GraphqlFallback,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RegionTransportError {
    pub failed_path: RegionTransportPath,
    pub fallback_attempted: bool,
    pub native_error: Option<String>,
    pub graphql_error: Option<String>,
}

impl RegionTransportError {
    #[cfg(test)]
    pub fn native_failed(error: ApiError) -> Self {
        Self {
            failed_path: RegionTransportPath::NativeServer,
            fallback_attempted: false,
            native_error: Some(error.to_string()),
            graphql_error: None,
        }
    }

    pub fn fallback_failed(native_error: ApiError, graphql_error: ApiError) -> Self {
        Self {
            failed_path: RegionTransportPath::GraphqlFallback,
            fallback_attempted: true,
            native_error: Some(native_error.to_string()),
            graphql_error: Some(graphql_error.to_string()),
        }
    }
}

impl Display for RegionTransportError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self.failed_path {
            RegionTransportPath::NativeServer => write!(
                f,
                "region native transport failed{}",
                self.native_error
                    .as_deref()
                    .map(|error| format!(": {error}"))
                    .unwrap_or_default()
            ),
            RegionTransportPath::GraphqlFallback => write!(
                f,
                "region GraphQL fallback failed after native transport error{}{}",
                self.native_error
                    .as_deref()
                    .map(|error| format!(" (native: {error})"))
                    .unwrap_or_default(),
                self.graphql_error
                    .as_deref()
                    .map(|error| format!(": {error}"))
                    .unwrap_or_default()
            ),
        }
    }
}

impl std::error::Error for RegionTransportError {}

impl From<&RegionTransportError> for RegionErrorEvidence {
    fn from(value: &RegionTransportError) -> Self {
        Self {
            failed_path: match value.failed_path {
                RegionTransportPath::NativeServer => RegionStorefrontErrorPath::NativeServer,
                RegionTransportPath::GraphqlFallback => RegionStorefrontErrorPath::GraphqlFallback,
            },
            fallback_attempted: value.fallback_attempted,
            native_error: value.native_error.clone(),
            graphql_error: value.graphql_error.clone(),
        }
    }
}

pub const DEFAULT_FALLBACK_POLICY: RegionFetchFallbackPolicy =
    RegionFetchFallbackPolicy::NativeThenGraphql;

pub async fn fetch_regions(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, RegionTransportError> {
    fetch_regions_with_policy(selected_region_id, locale, DEFAULT_FALLBACK_POLICY).await
}

pub async fn fetch_regions_with_policy(
    selected_region_id: Option<String>,
    locale: Option<String>,
    policy: RegionFetchFallbackPolicy,
) -> Result<StorefrontRegionsData, RegionTransportError> {
    match policy {
        RegionFetchFallbackPolicy::NativeThenGraphql => {
            match native_server_adapter::fetch_regions(selected_region_id.clone(), locale.clone())
                .await
            {
                Ok(data) => Ok(data),
                Err(native_error) => graphql_adapter::fetch_regions(selected_region_id, locale)
                    .await
                    .map_err(|graphql_error| {
                        RegionTransportError::fallback_failed(native_error, graphql_error)
                    }),
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

    #[test]
    fn fallback_error_keeps_native_and_graphql_evidence() {
        let error = RegionTransportError::fallback_failed(
            ApiError::ServerFn("tenant context missing".to_string()),
            ApiError::Graphql("network unavailable".to_string()),
        );

        assert_eq!(error.failed_path, RegionTransportPath::GraphqlFallback);
        assert!(error.fallback_attempted);
        assert_eq!(
            error.native_error.as_deref(),
            Some("tenant context missing")
        );
        assert_eq!(error.graphql_error.as_deref(), Some("network unavailable"));
        assert_eq!(
            error.to_string(),
            "region GraphQL fallback failed after native transport error (native: tenant context missing): network unavailable"
        );
    }

    #[test]
    fn transport_error_converts_to_ui_error_evidence() {
        let error = RegionTransportError::fallback_failed(
            ApiError::ServerFn("native failed".to_string()),
            ApiError::Graphql("graphql failed".to_string()),
        );
        let evidence = RegionErrorEvidence::from(&error);

        assert_eq!(
            evidence.failed_path,
            RegionStorefrontErrorPath::GraphqlFallback
        );
        assert!(evidence.fallback_attempted);
        assert_eq!(evidence.native_error.as_deref(), Some("native failed"));
        assert_eq!(evidence.graphql_error.as_deref(), Some("graphql failed"));
    }

    #[test]
    fn native_error_envelope_marks_fallback_as_not_attempted() {
        let error = RegionTransportError::native_failed(ApiError::ServerFn(
            "region/storefront-data requires the `ssr` feature".to_string(),
        ));

        assert_eq!(error.failed_path, RegionTransportPath::NativeServer);
        assert!(!error.fallback_attempted);
        assert_eq!(
            error.native_error.as_deref(),
            Some("region/storefront-data requires the `ssr` feature")
        );
        assert!(error.graphql_error.is_none());
    }
}
