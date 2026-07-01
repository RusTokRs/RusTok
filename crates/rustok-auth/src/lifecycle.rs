use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::Permission;
use rustok_core::{Locale, UserRole};
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct AuthLifecycleContext {
    pub tenant_id: Uuid,
    pub user_id: Option<Uuid>,
    pub session_id: Option<Uuid>,
    pub permissions: Vec<Permission>,
    pub locale: Locale,
}

#[derive(Clone, Debug)]
pub struct AuthUserRecord {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub role: UserRole,
    pub status: String,
    pub permissions: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct AuthTokenRecord {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub user: AuthUserRecord,
}

#[derive(Clone, Debug)]
pub struct AuthSessionRecord {
    pub id: Uuid,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub last_used_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug)]
pub struct AcceptInviteRecord {
    pub email: String,
    pub role: UserRole,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthLifecycleMutationError {
    EmailAlreadyExists,
    InvalidCredentials,
    UserInactive,
    InvalidRefreshToken,
    SessionExpired,
    UserNotFound,
    InvalidResetToken,
    InvalidInviteToken,
    Unauthorized,
    Validation(String),
    Internal(String),
}

impl std::fmt::Display for AuthLifecycleMutationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmailAlreadyExists => f.write_str("email already exists"),
            Self::InvalidCredentials => f.write_str("invalid credentials"),
            Self::UserInactive => f.write_str("user is inactive"),
            Self::InvalidRefreshToken => f.write_str("invalid refresh token"),
            Self::SessionExpired => f.write_str("session expired"),
            Self::UserNotFound => f.write_str("user not found"),
            Self::InvalidResetToken => f.write_str("invalid reset token"),
            Self::InvalidInviteToken => f.write_str("invalid invite token"),
            Self::Unauthorized => f.write_str("authentication required"),
            Self::Validation(message) | Self::Internal(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for AuthLifecycleMutationError {}

#[async_trait]
pub trait AuthLifecyclePort: Send + Sync {
    async fn current_user(
        &self,
        context: &AuthLifecycleContext,
    ) -> Result<AuthUserRecord, AuthLifecycleMutationError>;

    async fn list_sessions(
        &self,
        context: &AuthLifecycleContext,
        limit: u64,
    ) -> Result<Vec<AuthSessionRecord>, AuthLifecycleMutationError>;

    async fn sign_in(
        &self,
        context: &AuthLifecycleContext,
        email: String,
        password: String,
    ) -> Result<AuthTokenRecord, AuthLifecycleMutationError>;

    async fn sign_up(
        &self,
        context: &AuthLifecycleContext,
        email: String,
        password: String,
        name: Option<String>,
    ) -> Result<AuthTokenRecord, AuthLifecycleMutationError>;

    async fn refresh_token(
        &self,
        context: &AuthLifecycleContext,
        refresh_token: String,
    ) -> Result<AuthTokenRecord, AuthLifecycleMutationError>;

    async fn forgot_password(
        &self,
        context: &AuthLifecycleContext,
        email: String,
    ) -> Result<(), AuthLifecycleMutationError>;

    async fn update_profile(
        &self,
        context: &AuthLifecycleContext,
        name: Option<String>,
    ) -> Result<AuthUserRecord, AuthLifecycleMutationError>;

    async fn change_password(
        &self,
        context: &AuthLifecycleContext,
        current_password: String,
        new_password: String,
    ) -> Result<(), AuthLifecycleMutationError>;

    async fn reset_password(
        &self,
        context: &AuthLifecycleContext,
        token: String,
        new_password: String,
    ) -> Result<(), AuthLifecycleMutationError>;

    async fn logout(
        &self,
        context: &AuthLifecycleContext,
    ) -> Result<(), AuthLifecycleMutationError>;

    async fn revoke_session(
        &self,
        context: &AuthLifecycleContext,
        session_id: Uuid,
    ) -> Result<bool, AuthLifecycleMutationError>;

    async fn revoke_all_sessions(
        &self,
        context: &AuthLifecycleContext,
    ) -> Result<u64, AuthLifecycleMutationError>;

    async fn accept_invite(
        &self,
        context: &AuthLifecycleContext,
        token: String,
        password: String,
        name: Option<String>,
    ) -> Result<AcceptInviteRecord, AuthLifecycleMutationError>;
}

#[derive(Clone)]
pub struct AuthLifecycleRuntime {
    port: Arc<dyn AuthLifecyclePort>,
}

impl AuthLifecycleRuntime {
    pub fn new(port: Arc<dyn AuthLifecyclePort>) -> Self {
        Self { port }
    }

    pub fn port(&self) -> &dyn AuthLifecyclePort {
        self.port.as_ref()
    }
}
