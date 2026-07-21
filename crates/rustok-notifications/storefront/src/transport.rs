use crate::core::NotificationStorefrontState;

/// Returns the explicit degraded state until the owner inbox read API exists.
///
/// The host must not synthesize an unread count or persist a shadow inbox.
pub fn load_notification_storefront_state() -> NotificationStorefrontState {
    NotificationStorefrontState::foundation()
}
