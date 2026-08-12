const OWNER_GRAPHQL: &str = include_str!("../../src/graphql.rs");
const OWNER_MANIFEST: &str = include_str!("../../rustok-module.toml");
const OWNER_CARGO: &str = include_str!("../../Cargo.toml");
const STOREFRONT_GRAPHQL: &str = include_str!("../src/transport/graphql_adapter.rs");
const STOREFRONT_TRANSPORT: &str = include_str!("../src/transport.rs");
const STOREFRONT_UI: &str = include_str!("../src/ui/leptos.rs");
const SERVER_CARGO: &str = include_str!("../../../../apps/server/Cargo.toml");

#[test]
fn manifest_composes_owner_runtime_data_without_host_registry_code() {
    for marker in [
        "query = \"graphql::NotificationsQuery\"",
        "runtime_data_factory = \"graphql::attach_schema_data\"",
    ] {
        assert!(
            OWNER_MANIFEST.contains(marker),
            "manifest is missing `{marker}`"
        );
    }
    assert!(OWNER_CARGO.contains("default = []"));
    assert!(OWNER_CARGO.contains("server = [\"rustok-api/server\", \"dep:async-graphql\"]"));
    assert!(OWNER_CARGO.contains("async-graphql = { workspace = true, optional = true }"));
    assert!(SERVER_CARGO.contains("rustok-notifications/server"));
    assert!(OWNER_GRAPHQL.contains("pub fn attach_schema_data("));
    assert!(OWNER_GRAPHQL.contains("inputs.shared_get::<Arc<NotificationSourceRegistry>>()"));
    assert!(OWNER_GRAPHQL.contains("inputs.shared_get::<NotificationRecipientPolicyRuntime>()"));
    assert!(OWNER_GRAPHQL.contains("in_process_notification_inbox_storefront_port("));
}

#[test]
fn grouped_owner_queries_derive_identity_and_delegate_to_storefront_port() {
    for marker in [
        "async fn notification_inbox_group_summaries",
        "async fn notification_inbox_group_items",
        "let scope = authenticated_scope(ctx)?;",
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        ".list_group_summaries(",
        ".list_group_items(",
        "scope.port_context(\"group-summaries\")",
        "scope.port_context(\"group-items\")",
        "if !auth.is_human_user_principal()",
        "if auth.tenant_id != tenant.id",
        "actor: auth.port_actor()",
        ".with_deadline(GRAPHQL_READ_DEADLINE)",
        ".with_channel(\"storefront\")",
    ] {
        assert!(
            OWNER_GRAPHQL.contains(marker),
            "owner GraphQL is missing `{marker}`"
        );
    }

    let summaries_signature = OWNER_GRAPHQL
        .split("async fn notification_inbox_group_summaries")
        .nth(1)
        .and_then(|source| {
            source
                .split(") -> Result<GqlNotificationInboxGroupSummaryPage>")
                .next()
        })
        .expect("summary signature should exist");
    let items_signature = OWNER_GRAPHQL
        .split("async fn notification_inbox_group_items")
        .nth(1)
        .and_then(|source| {
            source
                .split(") -> Result<GqlNotificationInboxGroupItemsPage>")
                .next()
        })
        .expect("items signature should exist");
    for forbidden in ["tenant_id", "recipient_id", "user_id"] {
        assert!(!summaries_signature.contains(forbidden));
        assert!(!items_signature.contains(forbidden));
    }
}

#[test]
fn grouped_graphql_wire_is_bounded_and_transport_neutral() {
    for marker in [
        "query NotificationStorefrontGroupSummaries",
        "query NotificationStorefrontGroupItems",
        "$cursor: String",
        "$limit: Int",
        "$groupKey: String!",
        "$state: NotificationInboxItemState",
        "templateData { key value }",
        "load_group_summaries",
        "load_group_items",
    ] {
        assert!(
            STOREFRONT_GRAPHQL.contains(marker),
            "GraphQL adapter is missing `{marker}`"
        );
    }
    let production = STOREFRONT_GRAPHQL
        .split("#[cfg(test)]")
        .next()
        .expect("production GraphQL adapter section should exist");
    for forbidden in ["tenantId", "recipientId", "userId", "serde_json::Value"] {
        assert!(
            !production.contains(forbidden),
            "GraphQL adapter exposes `{forbidden}`"
        );
    }
}

#[test]
fn existing_grouped_ui_calls_selected_read_wrappers_only() {
    for marker in [
        "load_notification_unread_count_selected",
        "load_notification_group_summaries_selected",
        "load_notification_group_items_selected",
        "selected_storefront_read_transport_path",
        "UiTransportPath::NativeServer",
        "UiTransportPath::Graphql",
        "current_storefront_transport_context",
    ] {
        assert!(
            STOREFRONT_TRANSPORT.contains(marker),
            "transport is missing `{marker}`"
        );
    }
    for marker in [
        "load_notification_unread_count_selected",
        "load_notification_group_summaries(",
        "load_notification_group_items(",
        "authorize_notification_open",
        "apply_notification_group_state",
    ] {
        assert!(
            STOREFRONT_UI.contains(marker),
            "grouped UI is missing `{marker}`"
        );
    }
    assert!(!STOREFRONT_TRANSPORT.contains("fallback_failed"));
    assert!(STOREFRONT_TRANSPORT.contains("authorize_notification_open_selected"));
    assert!(STOREFRONT_TRANSPORT.contains("authorize_notification_open_native"));
    assert!(STOREFRONT_TRANSPORT.contains("apply_notification_group_state_selected"));
    assert!(STOREFRONT_TRANSPORT.contains("apply_notification_group_state_native"));
}
