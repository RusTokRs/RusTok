use async_graphql::{InputObject, SimpleObject};
use rustok_core::Permission;
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, SimpleObject, Serialize, Deserialize)]
pub struct AuthPayload {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in: i32,
    pub user: AuthUser,
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

pub(crate) fn auth_permission_strings(permissions: &[Permission]) -> Vec<String> {
    let mut values = permissions
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();
    values.sort();
    values.dedup();
    values
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

// --- Phase 1.5: new types ---

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
