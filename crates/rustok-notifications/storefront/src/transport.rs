mod native_server_adapter;

pub use native_server_adapter::{
    NativeNotificationStorefrontError, apply_notification_group_state,
    authorize_notification_open, load_notification_group_items,
    load_notification_group_summaries, load_notification_unread_count,
};

use crate::core::NotificationStorefrontState;

/// Returns the legacy explicit degraded sentinel for callers that have not composed
/// the grouped inbox resource yet.
///
/// `NotificationsView` no longer uses this sentinel. It reads the owner-backed native
/// transport and renders loading, empty, error, paging, and mutation states without
/// persisting a shadow inbox or inventing unread totals.
pub fn load_notification_storefront_state() -> NotificationStorefrontState {
    NotificationStorefrontState::foundation()
}
