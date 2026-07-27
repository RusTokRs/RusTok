const OWNER_GRAPHQL: &str = include_str!("../../src/graphql.rs");
const OWNER_MANIFEST: &str = include_str!("../../rustok-module.toml");
const OWNER_PORT: &str = include_str!("../../src/inbox_storefront_port.rs");
const STOREFRONT_GRAPHQL: &str = include_str!("../src/transport/graphql_adapter.rs");
const STOREFRONT_TRANSPORT: &str = include_str!("../src/transport.rs");
const STOREFRONT_UI: &str = include_str!("../src/ui/leptos.rs");

#[test]
fn owner_mutation_admits_before_bounded_command_and_reuses_port() {
    for marker in [
        "pub struct NotificationsMutation",
        "async fn notification_inbox_apply_group_state",
        "let scope = authenticated_scope(ctx)?;",
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "parse_idempotency_key(idempotency_key)?",
        "scope.write_port_context(\"group-state\", idempotency_key)",
        "NotificationInboxStorefrontGroupStateRequest",
        ".apply_group_state(",
        "GqlNotificationInboxGroupStateAction",
        "GqlNotificationInboxGroupStatePage",
        "GRAPHQL_WRITE_DEADLINE",
        "MAX_IDEMPOTENCY_KEY_BYTES",
        ".with_idempotency_key(idempotency_key)",
        ".with_channel(\"storefront\")",
    ] {
        assert!(
            OWNER_GRAPHQL.contains(marker),
            "owner GraphQL is missing `{marker}`"
        );
    }
    assert!(OWNER_MANIFEST.contains("mutation = \"graphql::NotificationsMutation\""));
    assert!(OWNER_PORT.contains("PortCallPolicy::write()"));
    assert!(OWNER_PORT.contains("NotificationInboxGroupStateService"));

    let signature = OWNER_GRAPHQL
        .split("async fn notification_inbox_apply_group_state")
        .nth(1)
        .and_then(|source| {
            source
                .split(") -> Result<GqlNotificationInboxGroupStatePage>")
                .next()
        })
        .expect("mutation signature should exist");
    for forbidden in ["tenant_id", "recipient_id", "user_id"] {
        assert!(!signature.contains(forbidden));
    }
}

#[test]
fn graphql_wire_carries_typed_action_idempotency_and_progress_only() {
    for marker in [
        "mutation NotificationStorefrontApplyGroupState",
        "$groupKey: String!",
        "$action: NotificationInboxGroupStateAction!",
        "$cursor: String",
        "$limit: Int",
        "$idempotencyKey: String!",
        "notificationInboxApplyGroupState",
        "GroupStateActionWire",
        "pub async fn apply_group_state",
        "scanned",
        "changed",
        "nextCursor",
        "hasMore",
    ] {
        assert!(
            STOREFRONT_GRAPHQL.contains(marker),
            "adapter is missing `{marker}`"
        );
    }
    let production = STOREFRONT_GRAPHQL
        .split("#[cfg(test)]")
        .next()
        .expect("production adapter should exist");
    for forbidden in ["tenantId", "recipientId", "userId", "serde_json::Value"] {
        assert!(!production.contains(forbidden));
    }
}

#[test]
fn selected_write_path_preserves_native_and_ui_without_fallback() {
    for marker in [
        "apply_notification_group_state as apply_notification_group_state_native",
        "selected_storefront_write_transport_path",
        "pub async fn apply_notification_group_state_selected",
        "notifications.storefront.group_state",
        "apply_notification_group_state_native(native_command)",
        "graphql_adapter::apply_group_state",
        "pub async fn apply_notification_group_state(",
        "current_storefront_transport_context()",
    ] {
        assert!(
            STOREFRONT_TRANSPORT.contains(marker),
            "transport is missing `{marker}`"
        );
    }
    assert!(!STOREFRONT_TRANSPORT.contains("fallback_failed"));
    assert!(STOREFRONT_UI.contains("apply_notification_group_state("));
    assert!(STOREFRONT_UI.contains("on_refresh.run(feedback)"));
}
