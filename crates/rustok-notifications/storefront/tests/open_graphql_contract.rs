const OWNER_GRAPHQL: &str = include_str!("../../src/graphql.rs");
const STOREFRONT_GRAPHQL: &str = include_str!("../src/transport/graphql_adapter.rs");
const STOREFRONT_TRANSPORT: &str = include_str!("../src/transport.rs");
const STOREFRONT_UI: &str = include_str!("../src/ui/leptos.rs");
const OWNER_PORT: &str = include_str!("../../src/inbox_storefront_port.rs");
const OWNER_OPEN: &str = include_str!("../../src/inbox.rs");

#[test]
fn owner_open_query_authenticates_before_bounded_identifier_validation() {
    for marker in [
        "async fn notification_inbox_authorize_open",
        "let scope = authenticated_scope(ctx)?;",
        "require_module_enabled(ctx, MODULE_SLUG).await?",
        "parse_notification_id(notification_id.as_str())?",
        ".authorize_open(",
        "scope.port_context(\"open\")",
        "NotificationInboxStorefrontOpenRequest { notification_id }",
        "if !auth.is_human_user_principal()",
        "if auth.tenant_id != tenant.id",
        "actor: auth.port_actor()",
        ".with_deadline(GRAPHQL_READ_DEADLINE)",
        ".with_channel(\"storefront\")",
        "MAX_NOTIFICATION_ID_BYTES",
        ".filter(|notification_id| !notification_id.is_nil())",
    ] {
        assert!(OWNER_GRAPHQL.contains(marker), "owner GraphQL is missing `{marker}`");
    }

    let open_resolver = OWNER_GRAPHQL
        .split("async fn notification_inbox_authorize_open")
        .nth(1)
        .and_then(|source| source.split("#[derive(Clone)]").next())
        .expect("open resolver should exist");
    let auth = open_resolver
        .find("let scope = authenticated_scope(ctx)?;")
        .expect("human-user admission should exist");
    let module = open_resolver
        .find("require_module_enabled(ctx, MODULE_SLUG).await?")
        .expect("module admission should exist");
    let parse = open_resolver
        .find("parse_notification_id(notification_id.as_str())?")
        .expect("bounded UUID validation should exist");
    let port = open_resolver
        .find(".authorize_open(")
        .expect("owner port delegation should exist");
    assert!(auth < module && module < parse && parse < port);

    let signature = OWNER_GRAPHQL
        .split("async fn notification_inbox_authorize_open")
        .nth(1)
        .and_then(|source| {
            source
                .split(") -> Result<GqlNotificationInboxOpenAuthorization>")
                .next()
        })
        .expect("open signature should exist");
    for forbidden in ["tenant_id", "recipient_id", "user_id"] {
        assert!(!signature.contains(forbidden));
    }
}

#[test]
fn open_decision_is_non_oracular_and_route_is_allowed_only() {
    for marker in [
        "pub enum GqlNotificationInboxOpenDecision",
        "Allowed",
        "Unavailable",
        "pub route: Option<String>",
        "NotificationInboxStorefrontOpenDecision::Allowed { route }",
        "route: Some(route.as_str().to_string())",
        "NotificationInboxStorefrontOpenDecision::Unavailable",
        "route: None",
    ] {
        assert!(OWNER_GRAPHQL.contains(marker), "owner GraphQL decision is missing `{marker}`");
    }
    for marker in [
        "find_by_id(request.notification_id)",
        "TenantId.eq(request.tenant_id)",
        "RecipientId.eq(request.recipient_id)",
        "NotificationRecipientPolicyDecision::Suppress",
        "NotificationOpenAuthorization::Unavailable",
    ] {
        assert!(OWNER_OPEN.contains(marker), "owner open service is missing `{marker}`");
    }
    assert!(OWNER_PORT.contains("PortCallPolicy::read()"));
    assert!(OWNER_PORT.contains("NotificationInboxOpenService"));
}

#[test]
fn storefront_open_graphql_request_exposes_only_notification_identity() {
    for marker in [
        "query NotificationStorefrontAuthorizeOpen",
        "$notificationId: String!",
        "notificationInboxAuthorizeOpen",
        "decision",
        "route",
        "OpenDecisionWire",
        "pub async fn authorize_open",
    ] {
        assert!(STOREFRONT_GRAPHQL.contains(marker), "GraphQL adapter is missing `{marker}`");
    }
    let production = STOREFRONT_GRAPHQL
        .split("#[cfg(test)]")
        .next()
        .expect("production GraphQL adapter should exist");
    for forbidden in ["tenantId", "recipientId", "userId", "serde_json::Value"] {
        assert!(!production.contains(forbidden), "open adapter exposes `{forbidden}`");
    }
    assert!(production.contains("OpenDecisionWire::Allowed"));
    assert!(production.contains("OpenDecisionWire::Unavailable"));
    assert!(production.contains("notification inbox open response is invalid"));
}

#[test]
fn open_transport_is_selected_without_fallback_and_ui_navigates_only_after_allowed() {
    for marker in [
        "authorize_notification_open as authorize_notification_open_native",
        "pub async fn authorize_notification_open_selected",
        "notifications.storefront.open_authorization",
        "selected_storefront_read_transport_path()",
        "authorize_notification_open_native(native_request)",
        "graphql_adapter::authorize_open",
        "pub async fn authorize_notification_open(",
        "current_storefront_transport_context()",
    ] {
        assert!(STOREFRONT_TRANSPORT.contains(marker), "selected open transport is missing `{marker}`");
    }
    assert!(!STOREFRONT_TRANSPORT.contains("fallback_failed"));
    assert!(STOREFRONT_TRANSPORT.contains("apply_notification_group_state,"));
    assert!(!STOREFRONT_TRANSPORT.contains("apply_notification_group_state_selected"));

    for marker in [
        "authorize_notification_open(NotificationStorefrontOpenRequest",
        "Ok(NotificationStorefrontOpenDecision::Allowed { route })",
        "navigate_to_route(route.as_str())",
        "Ok(NotificationStorefrontOpenDecision::Unavailable)",
    ] {
        assert!(STOREFRONT_UI.contains(marker), "grouped UI is missing `{marker}`");
    }
}
