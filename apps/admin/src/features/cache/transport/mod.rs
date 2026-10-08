mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;
use serde::Serialize;

use crate::features::cache::model::{
    CacheHealthResponse, PlatformSettingsResponse, UpdateSettingsInput, UpdateSettingsResponse,
};
use crate::shared::api::queries::{
    CACHE_HEALTH_QUERY, PLATFORM_SETTINGS_QUERY, UPDATE_PLATFORM_SETTINGS_MUTATION,
};
use crate::shared::api::{ApiError, request};

#[derive(Clone, Debug, Serialize)]
struct EmptyVariables {}

#[derive(Clone, Debug, Serialize)]
struct PlatformSettingsVariables {
    category: String,
}

#[derive(Clone, Debug, Serialize)]
struct UpdateSettingsVariables {
    input: UpdateSettingsInput,
}

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

async fn fetch_cache_health_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<CacheHealthResponse, ApiError> {
    request::<EmptyVariables, CacheHealthResponse>(
        CACHE_HEALTH_QUERY,
        EmptyVariables {},
        token,
        tenant_slug,
    )
    .await
}

pub async fn fetch_cache_health(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<CacheHealthResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::cache_health_native()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => fetch_cache_health_graphql(token, tenant_slug)
            .await
            .map_err(|error| error.to_string()),
    }
}

async fn fetch_cache_settings_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PlatformSettingsResponse, String> {
    request::<PlatformSettingsVariables, PlatformSettingsResponse>(
        PLATFORM_SETTINGS_QUERY,
        PlatformSettingsVariables {
            category: "cache".to_string(),
        },
        token,
        tenant_slug,
    )
    .await
    .map_err(|error| error.to_string())
}

pub async fn fetch_cache_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PlatformSettingsResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::cache_settings_native()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => fetch_cache_settings_graphql(token, tenant_slug).await,
    }
}

pub async fn update_cache_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
    settings: String,
) -> Result<bool, String> {
    request::<UpdateSettingsVariables, UpdateSettingsResponse>(
        UPDATE_PLATFORM_SETTINGS_MUTATION,
        UpdateSettingsVariables {
            input: UpdateSettingsInput {
                category: "cache".to_string(),
                settings,
            },
        },
        token,
        tenant_slug,
    )
    .await
    .map(|response| response.update_platform_settings.success)
    .map_err(|error| error.to_string())
}
