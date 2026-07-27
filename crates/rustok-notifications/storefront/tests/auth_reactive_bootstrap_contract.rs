const AUTH_CONTEXT: &str = include_str!("../../../leptos-auth/src/context.rs");
const STOREFRONT_TRANSPORT: &str = include_str!("../src/transport.rs");
const STOREFRONT_UI: &str = include_str!("../src/ui/leptos.rs");

#[test]
fn bootstrap_source_tracks_reactive_auth_transport_context() {
    for marker in [
        "pub fn current_notification_storefront_transport_context",
        "use_context::<AuthContext>()",
        "AuthContext::get_token",
        "AuthContext::get_tenant",
        "let transport_context =",
        "Memo::new(move |_| current_notification_storefront_transport_context())",
        "move || (refresh_nonce.get(), transport_context.get())",
        "move |(_, context)| async move { load_inbox_snapshot(context).await }",
    ] {
        assert!(
            STOREFRONT_TRANSPORT.contains(marker) || STOREFRONT_UI.contains(marker),
            "auth-reactive bootstrap source is missing `{marker}`"
        );
    }

    for marker in [
        "pub user: RwSignal<Option<AuthUser>>",
        "pub session: RwSignal<Option<AuthSession>>",
        "self.session.get().map(|s| s.token)",
        "self.session.get().map(|s| s.tenant)",
        "self.session.set(None)",
    ] {
        assert!(
            AUTH_CONTEXT.contains(marker),
            "AuthContext is missing tracked session behavior `{marker}`"
        );
    }
}

#[test]
fn bootstrap_reuses_one_exact_context_snapshot_and_clears_scope_feedback() {
    for marker in [
        "async fn load_inbox_snapshot(",
        "context: NotificationStorefrontTransportContext",
        "load_notification_unread_count_selected(context.clone())",
        "load_notification_group_summaries_selected(context,",
        "Effect::new(move |_|",
        "let _ = transport_context.get();",
        "set_refresh_feedback.set(None);",
        "set_refresh_nonce.update",
        "on_refresh.run(feedback)",
    ] {
        assert!(
            STOREFRONT_UI.contains(marker),
            "bootstrap context/refresh behavior is missing `{marker}`"
        );
    }

    let bootstrap = STOREFRONT_UI
        .split("async fn load_inbox_snapshot(")
        .nth(1)
        .and_then(|source| source.split("fn item_state_label").next())
        .expect("bootstrap helper should exist");
    assert!(!bootstrap.contains("load_notification_unread_count().await?"));
    assert!(!bootstrap.contains("current_notification_storefront_transport_context()"));
}

#[test]
fn auth_reactivity_adds_no_polling_storage_or_identity_rendering() {
    let production = STOREFRONT_UI
        .split("#[cfg(test)]")
        .next()
        .expect("storefront UI production source should exist");
    for forbidden in [
        "use_interval_fn",
        "set_interval",
        "localStorage",
        "gloo_storage",
        "data-access-token",
        "data-tenant-id",
        "data-recipient-id",
    ] {
        assert!(
            !production.contains(forbidden),
            "auth-reactive bootstrap must not add `{forbidden}`"
        );
    }
    assert!(STOREFRONT_TRANSPORT.contains("execute_selected_transport"));
    assert!(!STOREFRONT_TRANSPORT.contains("fallback_failed"));
}
