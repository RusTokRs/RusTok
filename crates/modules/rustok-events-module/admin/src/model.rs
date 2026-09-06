use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventDeliveryConfiguration {
    pub active_profile: String,
    pub desired_profile: String,
    pub iggy_mode: String,
    pub iggy_configured: bool,
    pub restart_required: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventDeliveryUpdate {
    pub desired_profile: String,
    pub restart_required: bool,
}
