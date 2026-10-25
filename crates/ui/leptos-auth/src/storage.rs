use crate::{AuthError, AuthSession, AuthUser};

#[cfg(target_arch = "wasm32")]
use crate::{ADMIN_SESSION_KEY, ADMIN_TENANT_KEY, ADMIN_TOKEN_KEY, ADMIN_USER_KEY};
#[cfg(target_arch = "wasm32")]
use gloo_storage::{LocalStorage, Storage};

/// Request-scoped authentication state supplied by an SSR host.
///
/// The host remains responsible for validating its cookie/session and constructing this snapshot.
/// `leptos-auth` only exposes it to existing `AuthContext` consumers, so UI modules do not branch
/// on browser vs server storage.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ServerAuthSnapshot {
    pub session: Option<AuthSession>,
    pub user: Option<AuthUser>,
}

#[cfg(not(target_arch = "wasm32"))]
pub fn provide_server_auth_snapshot(snapshot: ServerAuthSnapshot) {
    leptos::prelude::provide_context(snapshot);
}

#[cfg(target_arch = "wasm32")]
pub fn provide_server_auth_snapshot(_snapshot: ServerAuthSnapshot) {}

// ─── Browser storage ────────────────────────────────────────────────────────
//
// Credential material is never persisted in the browser. The access
// token and user profile live only in these in-memory cells (page
// lifetime); the refresh token lives in an HttpOnly cookie owned by
// the server (directly, for CSR, or via the SSR host's BFF cookie).
// The only LocalStorage entry kept is the tenant slug — not a
// credential — so a reload knows which tenant to silently refresh
// against. Keys written by the previous LocalStorage-based build are
// deleted on sight.

#[cfg(target_arch = "wasm32")]
mod memory {
    use std::cell::RefCell;

    use crate::{AuthSession, AuthUser};

    thread_local! {
        static SESSION: RefCell<Option<AuthSession>> = const { RefCell::new(None) };
        static USER: RefCell<Option<AuthUser>> = const { RefCell::new(None) };
    }

    pub fn save_session(session: AuthSession) {
        SESSION.with(|cell| *cell.borrow_mut() = Some(session));
    }

    pub fn load_session() -> Option<AuthSession> {
        SESSION.with(|cell| cell.borrow().clone())
    }

    pub fn save_user(user: AuthUser) {
        USER.with(|cell| *cell.borrow_mut() = Some(user));
    }

    pub fn load_user() -> Option<AuthUser> {
        USER.with(|cell| cell.borrow().clone())
    }

    pub fn clear() {
        SESSION.with(|cell| *cell.borrow_mut() = None);
        USER.with(|cell| *cell.borrow_mut() = None);
    }
}

/// Remove credential entries persisted by older builds that kept the
/// session in LocalStorage.
#[cfg(target_arch = "wasm32")]
fn purge_legacy_persisted_credentials() {
    LocalStorage::delete(ADMIN_SESSION_KEY);
    LocalStorage::delete(ADMIN_TOKEN_KEY);
    LocalStorage::delete(ADMIN_USER_KEY);
}

#[cfg(target_arch = "wasm32")]
pub fn save_session(session: &AuthSession) -> Result<(), AuthError> {
    purge_legacy_persisted_credentials();
    // The tenant slug is not a credential; persisting it lets a page
    // reload attempt a silent cookie refresh for the right tenant.
    let _ = LocalStorage::set(ADMIN_TENANT_KEY, &session.tenant);
    memory::save_session(session.clone());
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_session(_session: &AuthSession) -> Result<(), AuthError> {
    // Response-cookie persistence belongs to the SSR host because it owns cookie flags, domain,
    // session rotation, and CSRF policy.
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn load_session() -> Result<AuthSession, AuthError> {
    purge_legacy_persisted_credentials();
    memory::load_session().ok_or(AuthError::Unauthorized)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_session() -> Result<AuthSession, AuthError> {
    leptos::prelude::use_context::<ServerAuthSnapshot>()
        .and_then(|snapshot| snapshot.session)
        .ok_or(AuthError::Unauthorized)
}

#[cfg(target_arch = "wasm32")]
pub fn save_user(user: &AuthUser) -> Result<(), AuthError> {
    memory::save_user(user.clone());
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_user(_user: &AuthUser) -> Result<(), AuthError> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn load_user() -> Result<AuthUser, AuthError> {
    memory::load_user().ok_or(AuthError::Unauthorized)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_user() -> Result<AuthUser, AuthError> {
    leptos::prelude::use_context::<ServerAuthSnapshot>()
        .and_then(|snapshot| snapshot.user)
        .ok_or(AuthError::Unauthorized)
}

#[cfg(target_arch = "wasm32")]
pub fn clear_session() {
    purge_legacy_persisted_credentials();
    memory::clear();
}

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_session() {}

#[cfg(target_arch = "wasm32")]
pub fn get_token() -> Option<String> {
    memory::load_session().map(|session| session.token)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_token() -> Option<String> {
    leptos::prelude::use_context::<ServerAuthSnapshot>()
        .and_then(|snapshot| snapshot.session)
        .map(|session| session.token)
}

#[cfg(target_arch = "wasm32")]
pub fn get_tenant() -> Option<String> {
    memory::load_session()
        .map(|session| session.tenant)
        .or_else(|| LocalStorage::get(ADMIN_TENANT_KEY).ok())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_tenant() -> Option<String> {
    leptos::prelude::use_context::<ServerAuthSnapshot>()
        .and_then(|snapshot| snapshot.session)
        .map(|session| session.tenant)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_snapshot_is_plain_request_scoped_data() {
        let snapshot = ServerAuthSnapshot {
            session: Some(AuthSession {
                token: "token".to_string(),
                expires_at: 42,
                tenant: "demo".to_string(),
            }),
            user: Some(AuthUser {
                id: "user".to_string(),
                email: "user@example.test".to_string(),
                name: None,
                role: "admin".to_string(),
            }),
        };
        assert_eq!(snapshot.session.as_ref().unwrap().tenant, "demo");
        assert_eq!(snapshot.user.as_ref().unwrap().role, "admin");
    }
}
