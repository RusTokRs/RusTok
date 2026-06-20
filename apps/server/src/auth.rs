// Re-export types from rustok-auth (these don't need error conversion).
pub use rustok_auth::{
    AuthConfig, AuthError, AuthSettingsOverrides, Claims, EmailVerificationClaims, InviteClaims,
    JwtAlgorithm, PasswordResetClaims,
};

use loco_rs::app::AppContext;

use crate::error::{Error, Result};
use serde::Deserialize;

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

    let auth_settings = app_settings.and_then(|s| s.auth).unwrap_or_default();

    auth_config_from_parts(auth.secret.clone(), auth.expiration, auth_settings)
}

fn auth_config_from_parts(
    secret: String,
    access_expiration: u64,
    auth_settings: AuthSettingsOverrides,
) -> Result<AuthConfig> {
    rustok_auth::build_auth_config(secret, access_expiration, auth_settings).map_err(auth_err)
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

#[allow(clippy::too_many_arguments)]
pub fn encode_oauth_access_token(
    config: &AuthConfig,
    app_id: uuid::Uuid,
    tenant_id: uuid::Uuid,
    role: rustok_core::UserRole,
    client_id: uuid::Uuid,
    scopes: &[String],
    grant_type: &str,
    expires_in_secs: u64,
) -> Result<String> {
    rustok_auth::encode_oauth_access_token(
        config,
        rustok_auth::OauthAccessTokenInput {
            app_id,
            tenant_id,
            role,
            client_id,
            scopes,
            grant_type,
            expires_in_secs,
        },
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

pub fn encode_invite_token(
    config: &AuthConfig,
    tenant_id: uuid::Uuid,
    email: &str,
    role: rustok_core::UserRole,
    ttl_seconds: u64,
) -> Result<String> {
    rustok_auth::encode_invite_token(config, tenant_id, email, role, ttl_seconds).map_err(auth_err)
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

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn secret() -> String {
        "test-secret-key-for-unit-tests-only-32bytes!".to_string()
    }

    #[test]
    fn auth_config_defaults_to_hs256() {
        let config = auth_config_from_parts(secret(), 900, AuthSettingsOverrides::default())
            .expect("auth config");

        assert_eq!(config.algorithm, JwtAlgorithm::HS256);
        assert_eq!(config.access_expiration, 900);
        assert_eq!(config.refresh_expiration, 60 * 60 * 24 * 30);
        assert!(config.rsa_private_key_pem.is_none());
        assert!(config.rsa_public_key_pem.is_none());
    }

    #[test]
    fn auth_config_accepts_inline_rs256_keys() {
        let config = auth_config_from_parts(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                rsa_private_key_pem: Some("private".to_string()),
                rsa_public_key_pem: Some("public".to_string()),
                ..AuthSettingsOverrides::default()
            },
        )
        .expect("auth config");

        assert_eq!(config.algorithm, JwtAlgorithm::RS256);
        assert_eq!(config.rsa_private_key_pem.as_deref(), Some("private"));
        assert_eq!(config.rsa_public_key_pem.as_deref(), Some("public"));
    }

    #[test]
    fn auth_config_rejects_rs256_without_keys() {
        let result = auth_config_from_parts(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                ..AuthSettingsOverrides::default()
            },
        );

        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn auth_config_resolves_rs256_keys_from_env() {
        std::env::set_var("RUSTOK_TEST_RSA_PRIVATE", "private-from-env");
        std::env::set_var("RUSTOK_TEST_RSA_PUBLIC", "public-from-env");

        let config = auth_config_from_parts(
            secret(),
            900,
            AuthSettingsOverrides {
                algorithm: Some(JwtAlgorithm::RS256),
                rsa_private_key_env: Some("RUSTOK_TEST_RSA_PRIVATE".to_string()),
                rsa_public_key_env: Some("RUSTOK_TEST_RSA_PUBLIC".to_string()),
                ..AuthSettingsOverrides::default()
            },
        )
        .expect("auth config");

        assert_eq!(
            config.rsa_private_key_pem.as_deref(),
            Some("private-from-env")
        );
        assert_eq!(
            config.rsa_public_key_pem.as_deref(),
            Some("public-from-env")
        );

        std::env::remove_var("RUSTOK_TEST_RSA_PRIVATE");
        std::env::remove_var("RUSTOK_TEST_RSA_PUBLIC");
    }
}
