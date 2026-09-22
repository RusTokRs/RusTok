use std::sync::Arc;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
    async fn configuration(&self) -> Result<EventDeliveryConfigurationSnapshot, String>;

    async fn update_profile(
        &self,
        profile: String,
        actor_id: Uuid,
    ) -> Result<EventDeliveryUpdateOutcome, String>;
}

#[derive(Clone)]
pub struct SharedEventDeliveryControl(pub Arc<dyn EventDeliveryControl>);
