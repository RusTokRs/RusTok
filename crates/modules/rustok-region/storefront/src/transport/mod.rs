pub(crate) mod graphql_adapter;
pub(crate) mod native_server_adapter;

use std::fmt::{Display, Formatter};

use leptos::prelude::*;
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
use serde::{Deserialize, Serialize};

use crate::core::{RegionErrorEvidence, RegionStorefrontErrorPath};
use crate::model::StorefrontRegionsData;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) enum ApiError {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegionTransportPolicy {
    BuildProfileSelected,
}

pub type RegionTransportError = UiTransportError;

impl From<&RegionTransportError> for RegionErrorEvidence {
    fn from(value: &RegionTransportError) -> Self {
        Self {
            failed_path: match value.failed_path {
                UiTransportPath::NativeServer => RegionStorefrontErrorPath::NativeServer,
                UiTransportPath::Graphql => RegionStorefrontErrorPath::Graphql,
            },
            fallback_attempted: value.fallback_attempted,
            native_error: value.native_error.clone(),
            graphql_error: value.graphql_error.clone(),
        }
    }
}

pub const DEFAULT_TRANSPORT_POLICY: RegionTransportPolicy =
    RegionTransportPolicy::BuildProfileSelected;

fn selected_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn fetch_regions(
    selected_region_id: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontRegionsData, RegionTransportError> {
    fetch_regions_with_policy(selected_region_id, locale, DEFAULT_TRANSPORT_POLICY).await
}

pub async fn fetch_regions_with_policy(
    selected_region_id: Option<String>,
    locale: Option<String>,
    policy: RegionTransportPolicy,
) -> Result<StorefrontRegionsData, RegionTransportError> {
    match policy {
        RegionTransportPolicy::BuildProfileSelected => {
            let native_selected_region_id = selected_region_id.clone();
            let native_locale = locale.clone();
            execute_selected_transport(
                "region",
                selected_transport_path(),
                move || {
                    native_server_adapter::fetch_regions(native_selected_region_id, native_locale)
                },
                move || graphql_adapter::fetch_regions(selected_region_id, locale),
            )
            .await
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_policy_preserves_build_profile_transport_selection() {
        assert_eq!(
            DEFAULT_TRANSPORT_POLICY,
            RegionTransportPolicy::BuildProfileSelected
        );
    }

    #[test]
    fn default_test_profile_uses_graphql_transport_without_native_fallback() {
        assert_eq!(selected_transport_path(), UiTransportPath::Graphql);
    }

    #[test]
    fn fallback_error_keeps_native_and_graphql_evidence() {
        let error = RegionTransportError::fallback_failed(
            "region",
            ApiError::ServerFn("tenant context missing".to_string()),
            ApiError::Graphql("network unavailable".to_string()),
        );

        assert_eq!(error.failed_path, UiTransportPath::Graphql);
        assert!(error.fallback_attempted);
        assert_eq!(
            error.native_error.as_deref(),
            Some("tenant context missing")
        );
        assert_eq!(error.graphql_error.as_deref(), Some("network unavailable"));
        assert_eq!(
            error.to_string(),
            "region transport fallback failed: native_server=tenant context missing; graphql=network unavailable"
        );
    }

    #[test]
    fn transport_error_converts_to_ui_error_evidence() {
        let error = RegionTransportError::fallback_failed(
            "region",
            ApiError::ServerFn("native failed".to_string()),
            ApiError::Graphql("graphql failed".to_string()),
        );
        let evidence = RegionErrorEvidence::from(&error);

        assert_eq!(evidence.failed_path, RegionStorefrontErrorPath::Graphql);
        assert!(evidence.fallback_attempted);
        assert_eq!(evidence.native_error.as_deref(), Some("native failed"));
        assert_eq!(evidence.graphql_error.as_deref(), Some("graphql failed"));
    }

    #[test]
    fn native_error_envelope_marks_fallback_as_not_attempted() {
        let error = RegionTransportError::native(
            "region",
            ApiError::ServerFn("region/storefront-data requires the `ssr` feature".to_string()),
        );

        assert_eq!(error.failed_path, UiTransportPath::NativeServer);
        assert!(!error.fallback_attempted);
        assert_eq!(
            error.native_error.as_deref(),
            Some("region/storefront-data requires the `ssr` feature")
        );
        assert!(error.graphql_error.is_none());
    }
}
