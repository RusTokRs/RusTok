use async_trait::async_trait;
use rustok_api::{
    EventDeliveryConfigurationSnapshot, EventDeliveryControl, EventDeliveryUpdateOutcome,
};
use uuid::Uuid;

use crate::common::settings::EventDeliveryProfile;
use crate::services::event_delivery_settings_service::EventDeliverySettingsService;
use crate::services::event_transport_factory::EventRuntime;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Clone)]
pub struct ServerEventDeliveryControl {
    runtime: ServerRuntimeContext,
}

impl ServerEventDeliveryControl {
    pub fn new(runtime: ServerRuntimeContext) -> Self {
        Self { runtime }
    }
}

#[async_trait]
impl EventDeliveryControl for ServerEventDeliveryControl {
    async fn configuration(&self) -> Result<EventDeliveryConfigurationSnapshot, String> {
        let configuration = EventDeliverySettingsService::configuration(&self.runtime)
            .await
            .map_err(|error| error.to_string())?;
        let active_profile = self
            .runtime
            .shared_get::<std::sync::Arc<EventRuntime>>()
            .map(|runtime| runtime.delivery_profile)
            .unwrap_or(configuration.active_profile);
        let iggy = crate::services::iggy_connector_settings_service::IggyConnectorSettingsService::configuration(&self.runtime)
            .await
            .map_err(|error| error.to_string())?;

        Ok(EventDeliveryConfigurationSnapshot {
            active_profile: active_profile.as_str().to_string(),
            desired_profile: configuration.desired_profile.as_str().to_string(),
            iggy_mode: iggy.desired_mode,
            iggy_configured: configuration.iggy_configured,
            restart_required: active_profile != configuration.desired_profile,
        })
    }

    async fn update_profile(
        &self,
        profile: String,
        actor_id: Uuid,
    ) -> Result<EventDeliveryUpdateOutcome, String> {
        let profile = EventDeliveryProfile::parse(&profile).ok_or_else(|| {
            "profile must be one of: memory, outbox_local, outbox_iggy".to_string()
        })?;
        EventDeliverySettingsService::save_profile(&self.runtime, profile, actor_id)
            .await
            .map_err(|error| error.to_string())?;
        let active_profile = self
            .runtime
            .shared_get::<std::sync::Arc<EventRuntime>>()
            .map(|runtime| runtime.delivery_profile)
            .unwrap_or(self.runtime.settings().events.delivery_profile);

        Ok(EventDeliveryUpdateOutcome {
            desired_profile: profile.as_str().to_string(),
            restart_required: active_profile != profile,
        })
    }
}
