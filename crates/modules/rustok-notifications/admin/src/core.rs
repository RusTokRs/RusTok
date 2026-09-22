use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationsAdminPhase {
    SourceRegistry,
    Persistence,
    Delivery,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationsAdminStatus {
    pub phase: NotificationsAdminPhase,
    pub source_registry_ready: bool,
    pub persistence_ready: bool,
    pub delivery_ready: bool,
}

impl NotificationsAdminStatus {
    pub const fn foundation() -> Self {
        Self {
            phase: NotificationsAdminPhase::SourceRegistry,
            source_registry_ready: true,
            persistence_ready: false,
            delivery_ready: false,
        }
    }
}
