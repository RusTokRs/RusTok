mod graphql_adapter;
mod native_server_adapter;

pub use native_server_adapter::{
    NativeNotificationStorefrontError, apply_notification_group_state,
    authorize_notification_open, load_notification_group_items,
    load_notification_group_summaries, load_notification_unread_count,
};

use rustok_ui_transport::{UiTransportPath, UiTransportResult, execute_selected_transport};
use serde::{Deserialize, Serialize};

use crate::core::{
    NotificationStorefrontGroupItemsPage, NotificationStorefrontGroupItemsRequest,
    NotificationStorefrontGroupSummaryPage, NotificationStorefrontGroupSummaryRequest,
    NotificationStorefrontState, NotificationStorefrontUnreadCount,
};

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct NotificationStorefrontTransportContext {
    pub access_token: Option<String>,
    pub tenant_slug: Option<String>,
}

impl NotificationStorefrontTransportContext {
    pub fn new(access_token: Option<String>, tenant_slug: Option<String>) -> Self {
        Self {
            access_token,
            tenant_slug,
        }
    }
}

pub type NotificationNavigationTransportContext = NotificationStorefrontTransportContext;

fn selected_storefront_read_transport_path() -> UiTransportPath {
    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    {
        UiTransportPath::NativeServer
    }
    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    {
        UiTransportPath::Graphql
    }
}

pub async fn load_notification_unread_count_selected(
    context: NotificationStorefrontTransportContext,
) -> UiTransportResult<NotificationStorefrontUnreadCount> {
    let access_token = context.access_token;
    let tenant_slug = context.tenant_slug;
    execute_selected_transport(
        "notifications.storefront.unread_count",
        selected_storefront_read_transport_path(),
        load_notification_unread_count,
        move || graphql_adapter::load_navigation_unread_count(access_token, tenant_slug),
    )
    .await
}

pub async fn load_notification_group_summaries_selected(
    context: NotificationStorefrontTransportContext,
    request: NotificationStorefrontGroupSummaryRequest,
) -> UiTransportResult<NotificationStorefrontGroupSummaryPage> {
    let native_request = request.clone();
    let access_token = context.access_token;
    let tenant_slug = context.tenant_slug;
    execute_selected_transport(
        "notifications.storefront.group_summaries",
        selected_storefront_read_transport_path(),
        move || load_notification_group_summaries(native_request),
        move || graphql_adapter::load_group_summaries(access_token, tenant_slug, request),
    )
    .await
}

pub async fn load_notification_group_items_selected(
    context: NotificationStorefrontTransportContext,
    request: NotificationStorefrontGroupItemsRequest,
) -> UiTransportResult<NotificationStorefrontGroupItemsPage> {
    let native_request = request.clone();
    let access_token = context.access_token;
    let tenant_slug = context.tenant_slug;
    execute_selected_transport(
        "notifications.storefront.group_items",
        selected_storefront_read_transport_path(),
        move || load_notification_group_items(native_request),
        move || graphql_adapter::load_group_items(access_token, tenant_slug, request),
    )
    .await
}

pub async fn load_notification_navigation_unread_count(
    context: NotificationNavigationTransportContext,
) -> UiTransportResult<NotificationStorefrontUnreadCount> {
    load_notification_unread_count_selected(context).await
}

/// Returns the legacy explicit degraded sentinel for callers that have not composed
/// the grouped inbox resource yet.
///
/// `NotificationsView` no longer uses this sentinel. It reads the owner-backed transport
/// facade and renders loading, empty, error, paging, and mutation states without
/// persisting a shadow inbox or inventing unread totals.
pub fn load_notification_storefront_state() -> NotificationStorefrontState {
    NotificationStorefrontState::foundation()
}

#[cfg(test)]
mod tests {
    use super::{
        NotificationStorefrontTransportContext, selected_storefront_read_transport_path,
    };
    use rustok_ui_transport::UiTransportPath;

    #[cfg(not(any(feature = "ssr", feature = "hydrate")))]
    #[test]
    fn default_package_profile_selects_graphql_for_storefront_reads() {
        assert_eq!(
            selected_storefront_read_transport_path(),
            UiTransportPath::Graphql
        );
    }

    #[cfg(any(feature = "ssr", feature = "hydrate"))]
    #[test]
    fn integrated_package_profile_selects_native_storefront_reads() {
        assert_eq!(
            selected_storefront_read_transport_path(),
            UiTransportPath::NativeServer
        );
    }

    #[test]
    fn storefront_context_carries_only_transport_credentials() {
        let context = NotificationStorefrontTransportContext::new(
            Some("token".to_string()),
            Some("tenant".to_string()),
        );
        assert_eq!(context.access_token.as_deref(), Some("token"));
        assert_eq!(context.tenant_slug.as_deref(), Some("tenant"));
    }
}
