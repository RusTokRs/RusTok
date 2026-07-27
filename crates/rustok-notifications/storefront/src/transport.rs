mod graphql_adapter;
mod native_server_adapter;

pub use native_server_adapter::{
    NativeNotificationStorefrontError, apply_notification_group_state,
    authorize_notification_open, load_notification_group_items,
    load_notification_group_summaries, load_notification_unread_count,
};

use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};

use crate::core::{NotificationStorefrontState, NotificationStorefrontUnreadCount};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct NotificationNavigationTransportContext {
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl NotificationNavigationTransportContext {
    pub fn new(access_token: Option<String>, tenant_slug: Option<String>) -> Self {
        Self {
            access_token,
            tenant_slug,
        }
    }
}

fn selected_navigation_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn load_notification_navigation_unread_count(
    context: NotificationNavigationTransportContext,
) -> UiTransportResult<NotificationStorefrontUnreadCount> {
    let access_token = context.access_token;
    let tenant_slug = context.tenant_slug;
    execute_selected_transport(
        "notifications.storefront.navigation.unread_count",
        selected_navigation_transport_path(),
        load_notification_unread_count,
        move || graphql_adapter::load_navigation_unread_count(access_token, tenant_slug),
    )
    .await
}

/// Returns the legacy explicit degraded sentinel for callers that have not composed
/// the grouped inbox resource yet.
///
/// `NotificationsView` no longer uses this sentinel. It reads the owner-backed native
/// transport and renders loading, empty, error, paging, and mutation states without
/// persisting a shadow inbox or inventing unread totals.
pub fn load_notification_storefront_state() -> NotificationStorefrontState {
    NotificationStorefrontState::foundation()
}

#[cfg(test)]
mod tests {
    use super::{NotificationNavigationTransportContext, selected_navigation_transport_path};
    use rustok_ui_transport::UiTransportPath;

    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    #[test]
    fn default_package_profile_selects_graphql_for_headless_navigation() {
        assert_eq!(selected_navigation_transport_path(), UiTransportPath::Graphql);
    }

    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    #[test]
    fn integrated_package_profile_selects_native_navigation() {
        assert_eq!(
            selected_navigation_transport_path(),
            UiTransportPath::NativeServer
        );
    }

    #[test]
    fn navigation_context_carries_only_transport_credentials() {
        let context = NotificationNavigationTransportContext::new(
            Some("token".to_string()),
            Some("tenant".to_string()),
        );
        assert_eq!(context.access_token.as_deref(), Some("token"));
        assert_eq!(context.tenant_slug.as_deref(), Some("tenant"));
    }
}
