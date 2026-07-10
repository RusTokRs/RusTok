mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;
use serde_json::json;

use crate::features::dashboard::model::{DashboardStatsResponse, RecentActivityResponse};
use crate::shared::api::queries::{DASHBOARD_STATS_QUERY, RECENT_ACTIVITY_QUERY};
use crate::shared::api::request;

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

pub async fn fetch_dashboard_stats(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<DashboardStatsResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::dashboard_stats_native()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => request::<_, DashboardStatsResponse>(
            DASHBOARD_STATS_QUERY,
            json!({}),
            token,
            tenant_slug,
        )
        .await
        .map_err(|error| error.to_string()),
    }
}

pub async fn fetch_recent_activity(
    token: Option<String>,
    tenant_slug: Option<String>,
    limit: i64,
) -> Result<RecentActivityResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::recent_activity_native(limit)
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => request::<_, RecentActivityResponse>(
            RECENT_ACTIVITY_QUERY,
            json!({ "limit": limit }),
            token,
            tenant_slug,
        )
        .await
        .map_err(|error| error.to_string()),
    }
}
