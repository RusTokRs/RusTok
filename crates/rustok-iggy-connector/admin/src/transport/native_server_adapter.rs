use std::fmt::{Display, Formatter};

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::model::{IggyConnectorConfiguration, IggyConnectorForm, IggyConnectorUpdate};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
    Graphql(String),
}

impl Display for ApiError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) | Self::Graphql(error) => formatter.write_str(error),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

#[server(prefix = "/api/fn", endpoint = "iggy-connector/configuration")]
pub async fn iggy_connector_configuration_native()
-> Result<IggyConnectorConfiguration, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, Permission, has_effective_permission};
        use rustok_iggy_connector::SharedIggyConnectorControl;

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required"));
        }
        let control = runtime
            .shared_get::<SharedIggyConnectorControl>()
            .ok_or_else(|| ServerFnError::new("Iggy connector control is unavailable"))?;
        let snapshot = control
            .0
            .configuration()
            .await
            .map_err(ServerFnError::new)?;
        Ok(IggyConnectorConfiguration {
            active_mode: snapshot.active_mode,
            desired_mode: snapshot.desired_mode,
            bundled_available: snapshot.bundled_available,
            external_addresses: snapshot.external_addresses,
            external_username: snapshot.external_username,
            password_resolver: snapshot.password_resolver,
            password_key: snapshot.password_key,
            password_configured: snapshot.password_configured,
            tls_enabled: snapshot.tls_enabled,
            tls_domain: snapshot.tls_domain,
            configured: snapshot.configured,
            configuration_error: snapshot.configuration_error,
            restart_required: snapshot.restart_required,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-iggy-connector-admin requires the `ssr` feature for native configuration",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "iggy-connector/update")]
pub async fn update_iggy_connector_configuration_native(
    input: IggyConnectorForm,
) -> Result<IggyConnectorUpdate, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{AuthContext, Permission, has_effective_permission};
        use rustok_iggy_connector::{IggyConnectorSettingsInput, SharedIggyConnectorControl};

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE) {
            return Err(ServerFnError::new("settings:manage required"));
        }
        let control = runtime
            .shared_get::<SharedIggyConnectorControl>()
            .ok_or_else(|| ServerFnError::new("Iggy connector control is unavailable"))?;
        let outcome = control
            .0
            .update_configuration(
                IggyConnectorSettingsInput {
                    mode: input.mode,
                    external_addresses: input.external_addresses,
                    external_username: input.external_username,
                    password_resolver: input.password_resolver,
                    password_key: input.password_key,
                    tls_enabled: input.tls_enabled,
                    tls_domain: input.tls_domain,
                },
                auth.user_id,
                auth.tenant_id,
            )
            .await
            .map_err(ServerFnError::new)?;
        Ok(IggyConnectorUpdate {
            desired_mode: outcome.desired_mode,
            configured: outcome.configured,
            restart_required: outcome.restart_required,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "rustok-iggy-connector-admin requires the `ssr` feature for native updates",
        ))
    }
}

pub(super) async fn fetch_configuration() -> Result<IggyConnectorConfiguration, ApiError> {
    iggy_connector_configuration_native()
        .await
        .map_err(Into::into)
}

pub(super) async fn update_configuration(
    input: IggyConnectorForm,
) -> Result<IggyConnectorUpdate, ApiError> {
    update_iggy_connector_configuration_native(input)
        .await
        .map_err(Into::into)
}
