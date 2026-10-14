pub mod graphql_adapter;
pub mod native_server_adapter;

pub use graphql_adapter::ApiError;

use crate::model::*;

pub async fn fetch_overview(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<MessengerOverviewPayload, ApiError> {
    #[cfg(feature = "ssr")]
    {
        native_server_adapter::fetch_overview(token, tenant_slug)
            .await
            .map_err(|e| ApiError::Network(e))
    }
    #[cfg(not(feature = "ssr"))]
    {
        graphql_adapter::fetch_overview(token, tenant_slug).await
    }
}

pub async fn fetch_channels(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<ChannelListItemPayload>, ApiError> {
    #[cfg(feature = "ssr")]
    {
        native_server_adapter::fetch_channels(token, tenant_slug)
            .await
            .map_err(|e| ApiError::Network(e))
    }
    #[cfg(not(feature = "ssr"))]
    {
        graphql_adapter::fetch_channels(token, tenant_slug).await
    }
}

pub async fn fetch_e2ee_devices(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<Vec<E2eeDeviceAuditPayload>, ApiError> {
    #[cfg(feature = "ssr")]
    {
        native_server_adapter::fetch_e2ee_devices(token, tenant_slug)
            .await
            .map_err(|e| ApiError::Network(e))
    }
    #[cfg(not(feature = "ssr"))]
    {
        graphql_adapter::fetch_e2ee_devices(token, tenant_slug).await
    }
}
