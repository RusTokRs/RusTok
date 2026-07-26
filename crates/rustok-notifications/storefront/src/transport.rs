mod native_server_adapter;

pub use native_server_adapter::{
    NativeNotificationStorefrontError, apply_notification_group_state,
    authorize_notification_open, load_notification_group_items,
    load_notification_group_summaries, load_notification_unread_count,
};

use crate::core::NotificationStorefrontState;

/// Returns the explicit degraded UI state until the grouped inbox view is composed.
///
/// The native owner adapter is available to callers, but the current view must not
/// synthesize unread state, persist a shadow inbox, or claim mounted UI readiness.
pub fn load_notification_storefront_state() -> NotificationStorefrontState {
    NotificationStorefrontState::foundation()
}
