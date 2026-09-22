mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;
use serde::Serialize;

use crate::features::cache::model::CacheHealthResponse;
use crate::shared::api::queries::CACHE_HEALTH_QUERY;
use crate::shared::api::{ApiError, request};

#[derive(Clone, Debug, Serialize)]
struct EmptyVariables {}

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
