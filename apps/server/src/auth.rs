// Re-export types from rustok-auth (these don't need error conversion).
pub use rustok_auth::{
    AuthConfig, AuthError, AuthSettingsOverrides, Claims, EmailVerificationClaims, InviteClaims,
    PasswordResetClaims,
};

use loco_rs::app::AppContext;

use crate::error::{Error, Result};
use serde::Deserialize;

const DEFAULT_REFRESH_EXPIRATION_SECS: u64 = 60 * 60 * 24 * 30;

// ─── Loco bridge ─────────────────────────────────────────────────────
// Thin wrappers that convert `rustok_auth::AuthError` → `loco_rs::Error`.
// All server code imports from `crate::auth`, never directly from `rustok_auth`.

/// Build `AuthConfig` from Loco's `AppContext`.
pub fn auth_config_from_ctx(ctx: &AppContext) -> Result<AuthConfig> {
    let auth = ctx
        .config
        .auth
        .as_ref()
        .and_then(|auth| auth.jwt.as_ref())
        .ok_or_else(|| Error::InternalServerError)?;

    let app_settings = ctx
        .config
        .settings
        .as_ref()
        .and_then(|value| serde_json::from_value::<AppSettings>(value.clone()).ok());

    let auth_settings = app_settings.and_then(|s| s.auth);

    let refresh_expiration = auth_settings
        .as_ref()
        .and_then(|a| a.refresh_expiration)
        .unwrap_or(DEFAULT_REFRESH_EXPIRATION_SECS);

    let issuer = auth_settings
        .as_ref()
        .and_then(|a| a.issuer.clone())
        .unwrap_or_else(|| "rustok".to_string());

    let audience = auth_settings
        .and_then(|a| a.audience)
        .unwrap_or_else(|| "rustok-admin".to_string());

    Ok(AuthConfig {
        secret: auth.secret.clone(),
        access_expiration: auth.expiration,
        refresh_expiration,
        issuer,
        audience,
        algorithm: rustok_auth::JwtAlgorithm::HS256,
        rsa_private_key_pem: None,
        rsa_public_key_pem: None,
    })
}

// ─── Token functions ─────────────────────────────────────────────────

pub fn encode_access_token(
    config: &AuthConfig,
    user_id: uuid::Uuid,
    tenant_id: uuid::Uuid,
    role: rustok_core::UserRole,
    session_id: uuid::Uuid,
) -> Result<String> {
    rustok_auth::encode_access_token(config, user_id, tenant_id, role, session_id).map_err(auth_err)
}

pub fn encode_oauth_access_token(
    config: &AuthConfig,
    app_id: uuid::Uuid,
    tenant_id: uuid::Uuid,
    client_id: uuid::Uuid,
    scopes: &[String],
    grant_type: &str,
    expires_in_secs: u64,
) -> Result<String> {
    rustok_auth::encode_oauth_access_token(
        config,
        app_id,
        tenant_id,
        client_id,
        scopes,
        grant_type,
        expires_in_secs,
    )
    .map_err(auth_err)
}

pub fn decode_access_token(config: &AuthConfig, token: &str) -> Result<Claims> {
    rustok_auth::decode_access_token(config, token).map_err(auth_err)
}

pub fn encode_password_reset_token(
    config: &AuthConfig,
    tenant_id: uuid::Uuid,
    email: &str,
    ttl_seconds: u64,
) -> Result<String> {
    rustok_auth::encode_password_reset_token(config, tenant_id, email, ttl_seconds)
        .map_err(auth_err)
}

pub fn decode_password_reset_token(
    config: &AuthConfig,
    token: &str,
) -> Result<PasswordResetClaims> {
    rustok_auth::decode_password_reset_token(config, token).map_err(auth_err)
}

pub fn encode_email_verification_token(
    config: &AuthConfig,
    tenant_id: uuid::Uuid,
    email: &str,
    ttl_seconds: u64,
) -> Result<String> {
    rustok_auth::encode_email_verification_token(config, tenant_id, email, ttl_seconds)
        .map_err(auth_err)
}

pub fn decode_email_verification_token(
    config: &AuthConfig,
    token: &str,
) -> Result<EmailVerificationClaims> {
    rustok_auth::decode_email_verification_token(config, token).map_err(auth_err)
}

pub fn decode_invite_token(config: &AuthConfig, token: &str) -> Result<InviteClaims> {
    rustok_auth::decode_invite_token(config, token).map_err(auth_err)
}

// ─── Credential functions ────────────────────────────────────────────

pub fn hash_password(password: &str) -> Result<String> {
    rustok_auth::hash_password(password).map_err(auth_err)
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    rustok_auth::verify_password(password, password_hash).map_err(auth_err)
}

pub fn generate_refresh_token() -> String {
    rustok_auth::generate_refresh_token()
}

pub fn hash_refresh_token(token: &str) -> String {
    rustok_auth::hash_refresh_token(token)
}

// ─── Error conversion ────────────────────────────────────────────────

/// Convert `AuthError` → `loco_rs::Error`.
pub fn auth_err(err: AuthError) -> Error {
    match err {
        AuthError::InvalidCredentials | AuthError::InvalidAccessToken => {
            Error::Unauthorized(err.to_string())
        }
        AuthError::InvalidResetToken
        | AuthError::InvalidVerificationToken
        | AuthError::InvalidInviteToken => Error::Unauthorized(err.to_string()),
        AuthError::TokenEncodingFailed | AuthError::PasswordHashFailed => {
            Error::InternalServerError
        }
        AuthError::Internal(_) => Error::InternalServerError,
    }
}

#[derive(Debug, Deserialize)]
struct AppSettings {
    #[serde(default)]
    auth: Option<AuthSettingsOverrides>,
}
