use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NotificationInboxAvailability {
    Unavailable,
    Available,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontState {
    pub availability: NotificationInboxAvailability,
    pub unread_count: Option<u32>,
}

impl NotificationStorefrontState {
    pub const fn foundation() -> Self {
        Self {
            availability: NotificationInboxAvailability::Unavailable,
            unread_count: None,
        }
    }
}
