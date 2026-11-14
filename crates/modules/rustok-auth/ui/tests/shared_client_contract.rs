//! External-consumer contract for the Auth-owned FFA layers.

use rustok_auth_ui::core::{ADMIN_REFRESH_COOKIE, inspect_refresh_cookie_headers};
use rustok_auth_ui::model::{AuthSession, AuthUser, ClientAuthScope};
use rustok_auth_ui::ui::leptos::{AuthContext, ServerAuthSnapshot};

#[test]
fn owner_model_is_the_single_framework_neutral_session_contract() {
    let session = AuthSession {
        token: "not-a-real-token".into(),
        expires_at: 100,
        tenant: "tenant-a".into(),
    };
    let user = AuthUser {
        id: "user-a".into(),
        email: "user@example.test".into(),
        name: None,
        role: "member".into(),
        permissions: vec![],
    };
    // Assigning across package boundaries would fail if Auth had forked the model.
    let _: rustok_ui_auth::AuthSession = session.clone();
    let _: rustok_ui_auth::AuthUser = user.clone();
    let snapshot = ServerAuthSnapshot {
        session: Some(session),
        user: Some(user),
    };
    assert_eq!(snapshot.session.unwrap().tenant, "tenant-a");
    assert_eq!(snapshot.user.unwrap().id, "user-a");
}

#[test]
fn provider_and_consumers_share_the_same_scope_type_and_wire_cookie() {
    let _: fn(&AuthContext) -> ClientAuthScope = AuthContext::client_scope_untracked;
    assert!(ClientAuthScope::new(true).is_ready());
    assert!(!ClientAuthScope::new(false).is_ready());
    assert_eq!(ADMIN_REFRESH_COOKIE, "rustok-admin-refresh-v1");
    assert_eq!(inspect_refresh_cookie_headers([]), Ok(None));
    assert!(
        inspect_refresh_cookie_headers([
            "rustok-admin-refresh-v1=tenant-a.one",
            "rustok-admin-refresh-v1=tenant-a.two",
        ])
        .is_err()
    );
}

// Exercise the public provider and hooks as an external SSR consumer. The host is
// responsible for validating each snapshot; this test does not validate cookies or hydration.
#[cfg(all(feature = "ssr", not(target_arch = "wasm32")))]
mod ssr {
    use super::*;
    use leptos::prelude::*;
    use rustok_auth_ui::ui::leptos::{
        AuthProvider, provide_server_auth_snapshot, use_auth, use_current_user, use_tenant,
        use_token,
    };

    #[component]
    fn SnapshotConsumer() -> impl IntoView {
        let user = use_current_user()
            .get_untracked()
            .map(|user| user.id)
            .unwrap_or_else(|| "anonymous".to_string());
        let tenant = use_tenant()
            .get_untracked()
            .unwrap_or_else(|| "none".to_string());
        let has_token = use_token().get_untracked().is_some();
        let authenticated = use_auth().is_authenticated();
        view! { <output>{format!("{user}:{tenant}:{has_token}:{authenticated}")}</output> }
    }

    fn render(snapshot: ServerAuthSnapshot) -> String {
        let owner = Owner::new();
        owner.with(|| {
            provide_server_auth_snapshot(snapshot);
            view! { <AuthProvider><SnapshotConsumer /></AuthProvider> }.to_html()
        })
    }

    #[test]
    fn provider_reads_each_host_snapshot_in_a_fresh_owner() {
        let authenticated = render(ServerAuthSnapshot {
            session: Some(AuthSession {
                token: "not-a-real-token".into(),
                expires_at: i64::MAX,
                tenant: "tenant-a".into(),
            }),
            user: Some(AuthUser {
                id: "user-a".into(),
                email: "user@example.test".into(),
                name: None,
                role: "member".into(),
                permissions: vec![],
            }),
        });
        assert!(authenticated.contains("user-a:tenant-a:true:true"));
        assert!(!authenticated.contains("not-a-real-token"));

        let anonymous = render(ServerAuthSnapshot::default());
        assert!(anonymous.contains("anonymous:none:false:false"));
        assert!(!anonymous.contains("user-a"));
    }
}
