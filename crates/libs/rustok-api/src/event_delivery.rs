use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Error)]
pub enum EventDeliveryControlError {
    #[error("invalid event delivery profile: {0}")]
    InvalidProfile(String),
    #[error("event delivery configuration failed: {0}")]
    Configuration(String),
    #[error("event delivery profile update failed: {0}")]
    UpdateFailed(String),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventDeliveryConfigurationSnapshot {
    pub active_profile: String,
    pub desired_profile: String,
    pub iggy_mode: String,
    pub iggy_configured: bool,
    pub restart_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventDeliveryUpdateOutcome {
    pub desired_profile: String,
    pub restart_required: bool,
}

#[async_trait]
pub trait EventDeliveryControl: Send + Sync {
    async fn configuration(
        &self,
    ) -> Result<EventDeliveryConfigurationSnapshot, EventDeliveryControlError>;

    async fn update_profile(
        &self,
        profile: String,
        actor_id: Uuid,
    ) -> Result<EventDeliveryUpdateOutcome, EventDeliveryControlError>;
}

#[derive(Clone)]
pub struct SharedEventDeliveryControl(pub Arc<dyn EventDeliveryControl>);

impl std::fmt::Debug for SharedEventDeliveryControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedEventDeliveryControl").finish()
    }
}
