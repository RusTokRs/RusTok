use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};

use crate::{AuthSessionRecord, AuthTokenRecord, AuthUserRecord};

#[derive(Debug, Clone, InputObject)]
pub struct SignInInput {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, InputObject)]
pub struct SignUpInput {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct RefreshTokenInput {
    pub refresh_token: String,
}

#[derive(Debug, Clone, InputObject)]
pub struct ForgotPasswordInput {
    pub email: String,
}

#[derive(Debug, Clone, InputObject)]
pub struct ResetPasswordInput {
    pub token: String,
    pub new_password: String,
}

#[derive(Clone, SimpleObject, Serialize, Deserialize)]
pub struct AuthPayload {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i32,
    pub user: AuthUser,
}

impl std::fmt::Debug for AuthPayload {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthPayload")
            .field("access_token", &"<redacted>")
            .field("refresh_token", &"<redacted>")
            .field("token_type", &self.token_type)
            .field("expires_in", &self.expires_in)
            .field("user", &self.user)
            .finish()
    }
}

impl From<AuthTokenRecord> for AuthPayload {
    fn from(record: AuthTokenRecord) -> Self {
        Self {
            access_token: record.access_token,
            refresh_token: record.refresh_token,
            token_type: "Bearer".to_string(),
            expires_in: record.expires_in as i32,
            user: record.user.into(),
        }
    }
}

#[derive(Debug, Clone, SimpleObject, Serialize, Deserialize)]
pub struct AuthUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
    pub status: String,
    pub permissions: Vec<String>,
}

impl From<AuthUserRecord> for AuthUser {
    fn from(record: AuthUserRecord) -> Self {
        Self {
            id: record.id.to_string(),
            email: record.email,
            name: record.name,
            role: record.role.to_string(),
            status: record.status,
            permissions: record.permissions,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SignOutPayload {
    pub success: bool,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct ForgotPasswordPayload {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct ResetPasswordPayload {
    pub success: bool,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateProfileInput {
    pub name: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct ChangePasswordInput {
    pub current_password: String,
    pub new_password: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct ChangePasswordPayload {
    pub success: bool,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SessionItem {
    pub id: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_used_at: Option<String>,
    pub expires_at: String,
    pub created_at: String,
    pub current: bool,
}

impl SessionItem {
    pub fn from_record(record: AuthSessionRecord, current_session_id: Option<uuid::Uuid>) -> Self {
        Self {
            id: record.id.to_string(),
            ip_address: record.ip_address,
            user_agent: record.user_agent,
            last_used_at: record.last_used_at.map(|value| value.to_rfc3339()),
            expires_at: record.expires_at.to_rfc3339(),
            created_at: record.created_at.to_rfc3339(),
            current: current_session_id == Some(record.id),
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct SessionsPayload {
    pub sessions: Vec<SessionItem>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RevokeSessionPayload {
    pub success: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RevokeAllSessionsPayload {
    pub success: bool,
    pub revoked_count: i32,
}

#[derive(Debug, Clone, InputObject)]
pub struct AcceptInviteInput {
    pub token: String,
    pub password: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AcceptInvitePayload {
    pub success: bool,
    pub email: String,
    pub role: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_payload_debug_redacts_tokens() {
        let payload = AuthPayload {
            access_token: "super-secret-jwt-access-token".to_string(),
            refresh_token: "super-secret-jwt-refresh-token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            user: AuthUser {
                id: "user-123".to_string(),
                email: "user@example.com".to_string(),
                name: Some("User".to_string()),
                role: "admin".to_string(),
                status: "active".to_string(),
                permissions: vec!["users:read".to_string()],
            },
        };

        let formatted = format!("{payload:?}");
        assert!(!formatted.contains("super-secret-jwt-access-token"));
        assert!(!formatted.contains("super-secret-jwt-refresh-token"));
        assert!(formatted.contains("<redacted>"));
        assert!(formatted.contains("user@example.com"));
    }
}
