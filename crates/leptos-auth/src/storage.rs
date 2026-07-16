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

#[cfg(target_arch = "wasm32")]
pub fn save_session(session: &AuthSession) -> Result<(), AuthError> {
    LocalStorage::set(ADMIN_SESSION_KEY, session).map_err(|_| AuthError::Network)?;
    LocalStorage::set(ADMIN_TOKEN_KEY, &session.token).map_err(|_| AuthError::Network)?;
    LocalStorage::set(ADMIN_TENANT_KEY, &session.tenant).map_err(|_| AuthError::Network)?;
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
    LocalStorage::get(ADMIN_SESSION_KEY).map_err(|_| AuthError::Unauthorized)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_session() -> Result<AuthSession, AuthError> {
    leptos::prelude::use_context::<ServerAuthSnapshot>()
        .and_then(|snapshot| snapshot.session)
        .ok_or(AuthError::Unauthorized)
}

#[cfg(target_arch = "wasm32")]
pub fn save_user(user: &AuthUser) -> Result<(), AuthError> {
    LocalStorage::set(ADMIN_USER_KEY, user).map_err(|_| AuthError::Network)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn save_user(_user: &AuthUser) -> Result<(), AuthError> {
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn load_user() -> Result<AuthUser, AuthError> {
    LocalStorage::get(ADMIN_USER_KEY).map_err(|_| AuthError::Unauthorized)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn load_user() -> Result<AuthUser, AuthError> {
    leptos::prelude::use_context::<ServerAuthSnapshot>()
        .and_then(|snapshot| snapshot.user)
        .ok_or(AuthError::Unauthorized)
}

#[cfg(target_arch = "wasm32")]
pub fn clear_session() {
    LocalStorage::delete(ADMIN_SESSION_KEY);
    LocalStorage::delete(ADMIN_TOKEN_KEY);
    LocalStorage::delete(ADMIN_TENANT_KEY);
    LocalStorage::delete(ADMIN_USER_KEY);
}

#[cfg(not(target_arch = "wasm32"))]
pub fn clear_session() {}

#[cfg(target_arch = "wasm32")]
pub fn get_token() -> Option<String> {
    LocalStorage::get(ADMIN_TOKEN_KEY).ok()
}

#[cfg(not(target_arch = "wasm32"))]
pub fn get_token() -> Option<String> {
    leptos::prelude::use_context::<ServerAuthSnapshot>()
        .and_then(|snapshot| snapshot.session)
        .map(|session| session.token)
}

#[cfg(target_arch = "wasm32")]
pub fn get_tenant() -> Option<String> {
    LocalStorage::get(ADMIN_TENANT_KEY).ok()
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
                refresh_token: "refresh".to_string(),
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
