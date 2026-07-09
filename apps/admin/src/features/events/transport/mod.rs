mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;
use serde::Serialize;

use crate::features::events::model::{
    EventsStatusResponse, PlatformSettingsResponse, UpdateSettingsInput, UpdateSettingsResponse,
};
use crate::shared::api::queries::{
    EVENTS_STATUS_QUERY, PLATFORM_SETTINGS_QUERY, UPDATE_PLATFORM_SETTINGS_MUTATION,
};
use crate::shared::api::request;

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

pub async fn fetch_events_status(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<EventsStatusResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::events_status_native()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => request::<EmptyVariables, EventsStatusResponse>(
            EVENTS_STATUS_QUERY,
            EmptyVariables {},
            token,
            tenant_slug,
        )
        .await
        .map_err(|error| error.to_string()),
    }
}

pub async fn fetch_platform_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PlatformSettingsResponse, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::event_settings_native()
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => request::<PlatformSettingsVariables, PlatformSettingsResponse>(
            PLATFORM_SETTINGS_QUERY,
            PlatformSettingsVariables {
                category: "events".to_string(),
            },
            token,
            tenant_slug,
        )
        .await
        .map_err(|error| error.to_string()),
    }
}

pub async fn update_platform_settings(
    token: Option<String>,
    tenant_slug: Option<String>,
    settings: String,
) -> Result<bool, String> {
    request::<UpdateSettingsVariables, UpdateSettingsResponse>(
        UPDATE_PLATFORM_SETTINGS_MUTATION,
        UpdateSettingsVariables {
            input: UpdateSettingsInput {
                category: "events".to_string(),
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
