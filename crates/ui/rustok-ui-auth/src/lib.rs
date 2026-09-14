//! Shared client authentication values and expiry policy.
//! Server authentication and authorization remain owned by Auth and RBAC.

use serde::{Deserialize, Serialize};

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

impl AuthSession {
    /// Remaining lifetime relative to the host-supplied Unix time.
    pub fn secs_until_expiry(&self, now: i64) -> i64 {
        self.expires_at.saturating_sub(now)
    }

    /// Whether the session is within the client's existing refresh safety window.
    pub fn is_expired(&self, now: i64) -> bool {
        now >= self.expires_at.saturating_sub(60)
    }
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
