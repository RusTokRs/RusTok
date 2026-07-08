mod graphql_adapter;
mod native_server_adapter;

use crate::entities::oauth_app::model::OAuthApp;
use crate::shared::api::ApiError;
use rustok_ui_transport::UiTransportPath;
use uuid::Uuid;

pub use crate::features::oauth_apps::model::{
    CreateOAuthAppInput, CreateOAuthAppResult, UpdateOAuthAppInput,
};

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

pub async fn list_oauth_apps(
    token: Option<String>,
    tenant: Option<String>,
) -> Result<Vec<OAuthApp>, String> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::list_oauth_apps_native(100)
            .await
            .map_err(|error| error.to_string()),
        UiTransportPath::Graphql => graphql_adapter::list_oauth_apps(token, tenant)
            .await
            .map_err(|error| error.to_string()),
    }
}

pub async fn create_oauth_app(
    token: Option<String>,
    tenant: Option<String>,
    input: CreateOAuthAppInput,
) -> Result<CreateOAuthAppResult, ApiError> {
    graphql_adapter::create_oauth_app(token, tenant, input).await
}

pub async fn update_oauth_app(
    token: Option<String>,
    tenant: Option<String>,
    id: Uuid,
    input: UpdateOAuthAppInput,
) -> Result<OAuthApp, ApiError> {
    graphql_adapter::update_oauth_app(token, tenant, id, input).await
}

pub async fn rotate_oauth_app_secret(
    token: Option<String>,
    tenant: Option<String>,
    id: Uuid,
) -> Result<CreateOAuthAppResult, ApiError> {
    graphql_adapter::rotate_oauth_app_secret(token, tenant, id).await
}

pub async fn revoke_oauth_app(
    token: Option<String>,
    tenant: Option<String>,
    id: Uuid,
) -> Result<(), ApiError> {
    graphql_adapter::revoke_oauth_app(token, tenant, id).await
}
