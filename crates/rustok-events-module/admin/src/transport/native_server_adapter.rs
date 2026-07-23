use std::fmt::{Display, Formatter};

use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::model::{EventDeliveryConfiguration, EventDeliveryUpdate};

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

#[server(prefix = "/api/fn", endpoint = "events/configuration")]
pub async fn event_delivery_configuration_native(
) -> Result<EventDeliveryConfiguration, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            AuthContext, Permission, SharedEventDeliveryControl, has_effective_permission,
        };

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required"));
        }
        let control = runtime
            .shared_get::<SharedEventDeliveryControl>()
            .ok_or_else(|| ServerFnError::new("event delivery control is unavailable"))?;
        let snapshot = control
            .0
            .configuration()
            .await
            .map_err(ServerFnError::new)?;

        Ok(EventDeliveryConfiguration {
            active_profile: snapshot.active_profile,
            desired_profile: snapshot.desired_profile,
            iggy_mode: snapshot.iggy_mode,
            iggy_configured: snapshot.iggy_configured,
            restart_required: snapshot.restart_required,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-events-admin requires the `ssr` feature for native configuration",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "events/update-profile")]
pub async fn update_event_delivery_profile_native(
    profile: String,
) -> Result<EventDeliveryUpdate, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use rustok_api::{
            AuthContext, Permission, SharedEventDeliveryControl, has_effective_permission,
        };

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE) {
            return Err(ServerFnError::new("settings:manage required"));
        }
        let control = runtime
            .shared_get::<SharedEventDeliveryControl>()
            .ok_or_else(|| ServerFnError::new("event delivery control is unavailable"))?;
        let outcome = control
            .0
            .update_profile(profile, auth.user_id)
            .await
            .map_err(ServerFnError::new)?;

        Ok(EventDeliveryUpdate {
            desired_profile: outcome.desired_profile,
            restart_required: outcome.restart_required,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = profile;
        Err(ServerFnError::new(
            "rustok-events-admin requires the `ssr` feature for native updates",
        ))
    }
}

pub(super) async fn fetch_configuration() -> Result<EventDeliveryConfiguration, ApiError> {
    event_delivery_configuration_native()
        .await
        .map_err(Into::into)
}

pub(super) async fn update_profile(
    profile: String,
) -> Result<EventDeliveryUpdate, ApiError> {
    update_event_delivery_profile_native(profile)
        .await
        .map_err(Into::into)
}
