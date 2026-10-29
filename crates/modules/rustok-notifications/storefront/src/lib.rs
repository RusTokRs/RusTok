//! # rustok-notifications-storefront
//!
//! Leptos storefront UI package for the `rustok-notifications` module.
//!
//! Exposes the grouped notification inbox, unread badge, navigation header actions,
//! and notification management components.

pub mod core;
mod i18n;
pub mod transport;
pub mod ui;

pub use core::{
    NotificationInboxAvailability, NotificationStorefrontGroupItemsPage,
    NotificationStorefrontGroupItemsRequest, NotificationStorefrontGroupItemsSnapshot,
    NotificationStorefrontGroupStateAction, NotificationStorefrontGroupStateCommand,
    NotificationStorefrontGroupStatePage, NotificationStorefrontGroupSummary,
    NotificationStorefrontGroupSummaryPage, NotificationStorefrontGroupSummaryRequest,
    NotificationStorefrontInboxSnapshot, NotificationStorefrontItem,
    NotificationStorefrontItemState, NotificationStorefrontOpenDecision,
    NotificationStorefrontOpenRequest, NotificationStorefrontPriority,
    NotificationStorefrontState, NotificationStorefrontUnreadCount,
};
pub use transport::{
    NotificationNavigationTransportContext, NotificationStorefrontTransportContext,
    current_notification_storefront_transport_context, load_notification_storefront_state,
};
pub use ui::leptos::{NotificationUnreadBadge, NotificationsView};
pub use ui::navigation::NotificationNavigation;

#[cfg(test)]
mod contract_tests;
