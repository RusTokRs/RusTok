pub mod components;
pub mod context;
pub mod hooks;
pub mod storage;
pub mod transport;

pub mod api {
    pub use crate::transport::*;
}

use serde::{Deserialize, Serialize};

pub const ADMIN_TOKEN_KEY: &str = "rustok-admin-token";
pub const ADMIN_TENANT_KEY: &str = "rustok-admin-tenant";
pub const ADMIN_USER_KEY: &str = "rustok-admin-user";
pub const ADMIN_SESSION_KEY: &str = "rustok-admin-session";

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthSession {
    pub token: String,
    pub refresh_token: String,
    pub expires_at: i64,
    pub tenant: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, thiserror::Error)]
pub enum AuthError {
    #[error("Unauthorized")]
    Unauthorized,
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("Network error")]
    Network,
    #[error("HTTP error: {0}")]
    Http(u16),
}

impl AuthError {
    pub fn from_status(status: u16, is_login: bool) -> Self {
        match status {
            401 if is_login => Self::InvalidCredentials,
            401 => Self::Unauthorized,
            status => Self::Http(status),
        }
    }
}

pub use components::{GuestRoute, ProtectedRoute, RequireAuth};
pub use context::{AuthContext, AuthProvider};
pub use hooks::{
    use_auth, use_auth_error, use_current_user, use_is_authenticated, use_is_loading,
    use_is_token_valid, use_session, use_tenant, use_token,
};
pub use storage::{ServerAuthSnapshot, provide_server_auth_snapshot};
