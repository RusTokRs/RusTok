mod graphql_adapter;
mod native_server_adapter;

use rustok_ui_transport::UiTransportPath;

use crate::model::{IggyConnectorConfiguration, IggyConnectorForm, IggyConnectorUpdate};

pub use native_server_adapter::ApiError;

fn selected_transport_path() -> UiTransportPath {
    if cfg!(all(target_arch = "wasm32", not(feature = "hydrate"))) {
        UiTransportPath::Graphql
    } else {
        UiTransportPath::NativeServer
    }
}

pub async fn fetch_configuration(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<IggyConnectorConfiguration, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::fetch_configuration().await,
        UiTransportPath::Graphql => graphql_adapter::fetch_configuration(token, tenant_slug).await,
    }
}

pub async fn update_configuration(
    token: Option<String>,
    tenant_slug: Option<String>,
    input: IggyConnectorForm,
) -> Result<IggyConnectorUpdate, ApiError> {
    match selected_transport_path() {
        UiTransportPath::NativeServer => native_server_adapter::update_configuration(input).await,
        UiTransportPath::Graphql => {
            graphql_adapter::update_configuration(token, tenant_slug, input).await
        }
    }
}
