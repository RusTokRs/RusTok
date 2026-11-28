use async_graphql::{InputObject, SimpleObject};
use serde::{Deserialize, Serialize};

use crate::{AuthSessionRecord, AuthTokenRecord, AuthUserRecord};

#[derive(Clone, InputObject)]
pub struct SignInInput {
    pub email: String,
    pub password: String,
}

impl std::fmt::Debug for SignInInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignInInput")
            .field("email", &"<redacted>")
            .field("password", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, InputObject)]
pub struct SignUpInput {
    pub email: String,
    pub password: String,
    pub name: Option<String>,
}

impl std::fmt::Debug for SignUpInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SignUpInput")
            .field("email", &"<redacted>")
            .field("password", &"<redacted>")
            .field("name", &self.name.as_ref().map(|_| "<redacted>"))
            .finish()
    }
}

#[derive(Clone, InputObject)]
pub struct RefreshTokenInput {
    pub refresh_token: String,
}

impl std::fmt::Debug for RefreshTokenInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RefreshTokenInput")
            .field("refresh_token", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, InputObject)]
pub struct ForgotPasswordInput {
    pub email: String,
}

#[derive(Clone, InputObject)]
pub struct ResetPasswordInput {
    pub token: String,
    pub new_password: String,
}

impl std::fmt::Debug for ResetPasswordInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResetPasswordInput")
            .field("token", &"<redacted>")
            .field("new_password", &"<redacted>")
            .finish()
    }
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

#[derive(Clone, InputObject)]
pub struct RequestEmailChangeInput {
    pub new_email: String,
    pub current_password: String,
}

impl std::fmt::Debug for RequestEmailChangeInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RequestEmailChangeInput")
            .field("new_email", &"<redacted>")
            .field("current_password", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, InputObject)]
pub struct ConfirmEmailChangeInput {
    pub token: String,
}

impl std::fmt::Debug for ConfirmEmailChangeInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConfirmEmailChangeInput")
            .field("token", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct RequestEmailChangePayload {
    pub success: bool,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct ConfirmEmailChangePayload {
    pub success: bool,
    pub email: String,
}

#[derive(Clone, InputObject)]
pub struct ChangePasswordInput {
    pub current_password: String,
    pub new_password: String,
}

impl std::fmt::Debug for ChangePasswordInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ChangePasswordInput")
            .field("current_password", &"<redacted>")
            .field("new_password", &"<redacted>")
            .finish()
    }
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

#[derive(Clone, InputObject)]
pub struct AcceptInviteInput {
    pub token: String,
    pub password: String,
    pub name: Option<String>,
}

impl std::fmt::Debug for AcceptInviteInput {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AcceptInviteInput")
            .field("token", &"<redacted>")
            .field("password", &"<redacted>")
            .field("name", &self.name.as_ref().map(|_| "<redacted>"))
            .finish()
    }
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

    fn assert_redacted(value: &impl std::fmt::Debug, secrets: &[&str]) {
        for formatted in [format!("{value:?}"), format!("{value:#?}")] {
            assert!(formatted.contains("<redacted>"));
            for secret in secrets {
                assert!(!formatted.contains(*secret), "credential exposed in Debug");
            }
        }
    }

    #[test]
    fn credential_inputs_debug_redacts_secrets_and_identity_fields() {
        assert_redacted(
            &SignInInput {
                email: "SignInInput-email-sentinel".to_string(),
                password: "SignInInput-password-sentinel".to_string(),
            },
            &[
                "SignInInput-email-sentinel",
                "SignInInput-password-sentinel",
            ],
        );
        assert_redacted(
            &SignUpInput {
                email: "SignUpInput-email-sentinel".to_string(),
                password: "SignUpInput-password-sentinel".to_string(),
                name: Some("SignUpInput-name-sentinel".to_string()),
            },
            &[
                "SignUpInput-email-sentinel",
                "SignUpInput-password-sentinel",
                "SignUpInput-name-sentinel",
            ],
        );
        assert_redacted(
            &RefreshTokenInput {
                refresh_token: "RefreshTokenInput-refresh_token-sentinel".to_string(),
            },
            &[
                "RefreshTokenInput-refresh_token-sentinel",
            ],
        );
        assert_redacted(
            &ResetPasswordInput {
                token: "ResetPasswordInput-token-sentinel".to_string(),
                new_password: "ResetPasswordInput-new_password-sentinel".to_string(),
            },
            &[
                "ResetPasswordInput-token-sentinel",
                "ResetPasswordInput-new_password-sentinel",
            ],
        );
        assert_redacted(
            &RequestEmailChangeInput {
                new_email: "RequestEmailChangeInput-new_email-sentinel".to_string(),
                current_password: "RequestEmailChangeInput-current_password-sentinel".to_string(),
            },
            &[
                "RequestEmailChangeInput-new_email-sentinel",
                "RequestEmailChangeInput-current_password-sentinel",
            ],
        );
        assert_redacted(
            &ConfirmEmailChangeInput {
                token: "ConfirmEmailChangeInput-token-sentinel".to_string(),
            },
            &[
                "ConfirmEmailChangeInput-token-sentinel",
            ],
        );
        assert_redacted(
            &ChangePasswordInput {
                current_password: "ChangePasswordInput-current_password-sentinel".to_string(),
                new_password: "ChangePasswordInput-new_password-sentinel".to_string(),
            },
            &[
                "ChangePasswordInput-current_password-sentinel",
                "ChangePasswordInput-new_password-sentinel",
            ],
        );
        assert_redacted(
            &AcceptInviteInput {
                token: "AcceptInviteInput-token-sentinel".to_string(),
                password: "AcceptInviteInput-password-sentinel".to_string(),
                name: Some("AcceptInviteInput-name-sentinel".to_string()),
            },
            &[
                "AcceptInviteInput-token-sentinel",
                "AcceptInviteInput-password-sentinel",
                "AcceptInviteInput-name-sentinel",
            ],
        );
    }

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
