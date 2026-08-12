const MANIFEST: &str = include_str!("../../rustok-module.toml");
const OWNER_GRAPHQL: &str = include_str!("../../src/graphql.rs");
const GRAPHQL_ADAPTER: &str = include_str!("../src/transport/graphql_adapter.rs");
const TRANSPORT: &str = include_str!("../src/transport.rs");
const NAVIGATION: &str = include_str!("../src/ui/navigation.rs");
const LIBRARY: &str = include_str!("../src/lib.rs");
const HOST_APP: &str = include_str!("../../../../apps/storefront/src/app/mod.rs");
const HOST_HEADER: &str = include_str!("../../../../apps/storefront/src/widgets/header/mod.rs");
const HOST_BUILD: &str = include_str!("../../../../apps/storefront/build.rs");

#[test]
fn manifest_registers_module_owned_header_action_without_host_imports() {
    for marker in [
        "[provides.graphql]",
        "query = \"graphql::NotificationsQuery\"",
        "id = \"notifications-header-action\"",
        "component = \"NotificationNavigation\"",
        "slot = \"header_actions\"",
        "leptos_locales_path = \"storefront/locales\"",
    ] {
        assert!(MANIFEST.contains(marker), "manifest is missing `{marker}`");
    }
    assert!(LIBRARY.contains("pub use ui::navigation::NotificationNavigation;"));
    assert!(HOST_BUILD.contains("StorefrontSlot::HeaderActions"));
    assert!(HOST_APP.contains("components_for_slot(StorefrontSlot::HeaderActions"));
    assert!(HOST_HEADER.contains("action_views: Vec<AnyView>"));
    assert!(HOST_HEADER.contains("data-storefront-header-actions"));
    assert!(!HOST_APP.contains("rustok_notifications_storefront::NotificationNavigation"));
}

#[test]
fn navigation_uses_context_route_and_best_effort_exact_count() {
    for marker in [
        "module_route_base(\"notifications\")",
        "Resource::new_blocking",
        "AuthContext::get_token",
        "AuthContext::get_tenant",
        "load_notification_navigation_unread_count",
        "NotificationUnreadBadge",
        "let unread_count = count.unread_count",
        "unread_count > 0",
        "data-notification-navigation=\"unavailable\"",
    ] {
        assert!(
            NAVIGATION.contains(marker),
            "navigation is missing `{marker}`"
        );
    }
    assert!(!NAVIGATION.contains("localStorage"));
    assert!(!NAVIGATION.contains("window.location"));
    assert!(!NAVIGATION.contains("/modules/notifications"));
}

#[test]
fn unread_count_transport_is_dual_path_without_identity_payload() {
    for marker in [
        "UiTransportPath::NativeServer",
        "UiTransportPath::Graphql",
        "execute_selected_transport",
        "load_notification_unread_count_selected",
        "load_notification_navigation_unread_count",
        "graphql_adapter::load_navigation_unread_count",
        "notifications.storefront.unread_count",
    ] {
        assert!(
            TRANSPORT.contains(marker),
            "transport is missing `{marker}`"
        );
    }
    for marker in [
        "notificationInboxUnreadCount",
        "unreadCount",
        "access_token",
        "tenant_slug",
    ] {
        assert!(
            GRAPHQL_ADAPTER.contains(marker),
            "GraphQL adapter is missing `{marker}`"
        );
    }
    let production = GRAPHQL_ADAPTER
        .split("#[cfg(test)]")
        .next()
        .expect("GraphQL adapter production section should exist");
    for forbidden in ["tenantId", "recipientId", "userId"] {
        assert!(
            !production.contains(forbidden),
            "GraphQL navigation request must not expose `{forbidden}`"
        );
    }
}

#[test]
fn owner_graphql_derives_scope_and_sanitizes_failures() {
    for marker in [
        "ctx.data_opt::<AuthContext>()",
        "if !auth.is_human_user_principal()",
        "ctx.data_opt::<TenantContext>()",
        "if auth.tenant_id != tenant.id",
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "tenant_id: scope.tenant_id",
        "recipient_id: scope.recipient_id",
        "actor: auth.port_actor()",
        "NotificationInboxUnreadCountService::new(db)",
        "NOTIFICATION_INBOX_UNAVAILABLE",
        "PUBLIC_UNAVAILABLE_MESSAGE",
        "other.is_retryable()",
    ] {
        assert!(
            OWNER_GRAPHQL.contains(marker),
            "owner GraphQL is missing `{marker}`"
        );
    }
    assert!(!OWNER_GRAPHQL.contains("async_graphql::Error::new(error.to_string())"));
    assert!(!OWNER_GRAPHQL.contains("format!(\"{error}\")"));
}
