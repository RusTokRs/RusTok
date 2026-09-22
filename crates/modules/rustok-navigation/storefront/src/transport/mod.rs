mod graphql_adapter;
mod native_server_adapter;
use crate::model::{StorefrontMenu, StorefrontMenuLocation};
use leptos::prelude::ServerFnError;
use rustok_ui_transport::{UiTransportError, UiTransportPath, execute_selected_transport};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}
impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) | Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}
impl std::error::Error for ApiError {}
impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}
pub type NavigationTransportError = UiTransportError;
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
pub async fn fetch_active_menu(
    location: StorefrontMenuLocation,
    locale: Option<String>,
) -> Result<Option<StorefrontMenu>, NavigationTransportError> {
    let native_locale = locale.clone();
    execute_selected_transport(
        "navigation",
        selected_transport_path(),
        move || {
            native_server_adapter::fetch_active_menu_server(
                configured_tenant_slug(),
                location,
                native_locale,
            )
        },
        move || graphql_adapter::fetch_active_menu(location, locale),
    )
    .await
}
pub(crate) fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let value = value.trim().to_string();
            (!value.is_empty()).then_some(value)
        })
    })
}
