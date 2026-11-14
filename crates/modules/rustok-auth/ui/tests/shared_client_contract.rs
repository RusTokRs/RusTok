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
