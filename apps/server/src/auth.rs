// Re-export types from rustok-auth (these don't need error conversion).
pub use rustok_auth::{
    AuthConfig, AuthError, AuthSettingsOverrides, Claims, DEFAULT_MIN_PASSWORD_LENGTH_CHARS,
    InviteClaims, MAX_PASSWORD_LENGTH_BYTES, MIN_PASSWORD_LENGTH_FLOOR_CHARS,
    normalize_display_name, normalize_email, password_hash_needs_rehash,
    validate_password_policy,
};

use crate::error::{Error, Result};
use serde::Deserialize;
use rustok_auth::{AuthMacPurpose, auth_purpose_mac, is_canonical_auth_mac_tag};
use subtle::ConstantTimeEq;

const TOKEN_SUBJECT_SEPARATOR: char = '\u{001f}';

/// Server-owned password reset claims.
///
/// The generic auth crate validates the JWT envelope and purpose. The server
/// additionally binds the token to the password hash that existed when the
/// reset was requested, making the token invalid after the first successful
/// password change on every server instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasswordResetClaims {
    pub sub: String,
    pub tenant_id: uuid::Uuid,
    pub credential_fingerprint: String,
}

/// Server-owned email verification claims bound to a concrete account id.
///
/// Binding the token to `user_id` prevents a token issued for a deleted account
/// from verifying a newly created account that reuses the same email address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmailVerificationClaims {
    pub sub: String,
    pub tenant_id: uuid::Uuid,
    pub user_id: uuid::Uuid,
}

// ─── Server adapter ───────────────────────────────────────────────────
// Thin wrappers that convert `rustok_auth::AuthError` to the server error bridge.
// All server code imports from `crate::auth`, never directly from `rustok_auth`.

fn parse_auth_settings(settings: Option<&serde_json::Value>) -> Result<AuthSettingsOverrides> {
    let app_settings = settings
        .map(|value| serde_json::from_value::<AppSettings>(value.clone()))
        .transpose()?;
    Ok(app_settings.and_then(|s| s.auth).unwrap_or_default())
}

/// The file source supplies all JWT keys; never build an unused startup signer.
pub(crate) fn auth_config_from_host_document(
    mac_root: String,
    access_expiration: u64,
    settings: Option<&serde_json::Value>,
    bytes: &[u8],
) -> Result<(u64, AuthConfig)> {
    rustok_auth::build_auth_config_from_document(
        mac_root, access_expiration, parse_auth_settings(settings)?, bytes,
    ).map_err(auth_err)
}

/// Fresh, complete RSA configuration for an isolated server test runtime.
/// Tests sharing one security store must clone this configuration rather
/// than generating another one for token issuance.
#[cfg(test)]
pub(crate) fn test_auth_config(mac_root: String) -> AuthConfig {
    let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().expect("RSA fixture");
    auth_config_from_host_document(mac_root, 900, None, fixture.document(1).to_string().as_bytes())
        .expect("valid test JWT configuration").1
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
    token_id: uuid::Uuid,
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
            token_id,
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
    password_hash: &str,
    ttl_seconds: u64,
) -> Result<String> {
    let normalized_email = email.trim().to_lowercase();
    let fingerprint = password_reset_credential_fingerprint(config, password_hash)?;
    let subject = format!("{normalized_email}{TOKEN_SUBJECT_SEPARATOR}{fingerprint}");
    rustok_auth::encode_password_reset_token(config, tenant_id, &subject, ttl_seconds)
        .map_err(auth_err)
}

pub fn decode_password_reset_token(
    config: &AuthConfig,
    token: &str,
) -> Result<PasswordResetClaims> {
    let claims = rustok_auth::decode_password_reset_token(config, token).map_err(auth_err)?;
    let (email, credential_fingerprint) = claims
        .sub
        .rsplit_once(TOKEN_SUBJECT_SEPARATOR)
        .filter(|(email, fingerprint)| !email.is_empty() && is_canonical_auth_mac_tag(fingerprint))
        .ok_or_else(|| auth_err(AuthError::InvalidResetToken))?;

    Ok(PasswordResetClaims {
        sub: email.to_string(),
        tenant_id: claims.tenant_id,
        credential_fingerprint: credential_fingerprint.to_string(),
    })
}

pub fn password_reset_credential_matches(
    config: &AuthConfig,
    password_hash: &str,
    expected_fingerprint: &str,
) -> bool {
    if !is_canonical_auth_mac_tag(expected_fingerprint) {
        return false;
    }
    let Ok(actual) = password_reset_credential_fingerprint(config, password_hash) else {
        return false;
    };
    actual
        .as_bytes()
        .ct_eq(expected_fingerprint.as_bytes())
        .into()
}

fn password_reset_credential_fingerprint(config: &AuthConfig, password_hash: &str) -> Result<String> {
    let tag = auth_purpose_mac(config, AuthMacPurpose::PasswordResetCredential, password_hash.as_bytes())
        .map_err(auth_err)?;
    Ok(hex::encode(tag))
}

pub fn encode_email_verification_token(
    config: &AuthConfig,
    tenant_id: uuid::Uuid,
    user_id: uuid::Uuid,
    email: &str,
    ttl_seconds: u64,
) -> Result<String> {
    let normalized_email = email.trim().to_lowercase();
    let subject = format!("{user_id}{TOKEN_SUBJECT_SEPARATOR}{normalized_email}");
    rustok_auth::encode_email_verification_token(config, tenant_id, &subject, ttl_seconds)
        .map_err(auth_err)
}

pub fn decode_email_verification_token(
    config: &AuthConfig,
    token: &str,
) -> Result<EmailVerificationClaims> {
    let claims = rustok_auth::decode_email_verification_token(config, token).map_err(auth_err)?;
    let (user_id, email) = claims
        .sub
        .split_once(TOKEN_SUBJECT_SEPARATOR)
        .filter(|(user_id, email)| !user_id.is_empty() && !email.is_empty())
        .ok_or_else(|| auth_err(AuthError::InvalidVerificationToken))?;
    let user_id = uuid::Uuid::parse_str(user_id)
        .map_err(|_| auth_err(AuthError::InvalidVerificationToken))?;

    Ok(EmailVerificationClaims {
        sub: email.to_string(),
        tenant_id: claims.tenant_id,
        user_id,
    })
}

pub fn encode_invite_token(
    config: &AuthConfig,
    tenant_id: uuid::Uuid,
    email: &str,
    role: rustok_core::UserRole,
    custom_role_slug: Option<String>,
    ttl_seconds: u64,
) -> Result<String> {
    rustok_auth::encode_invite_token(
        config,
        tenant_id,
        email,
        role,
        custom_role_slug,
        ttl_seconds,
    )
    .map_err(auth_err)
}

pub fn decode_invite_token(config: &AuthConfig, token: &str) -> Result<InviteClaims> {
    rustok_auth::decode_invite_token(config, token).map_err(auth_err)
}

// ─── Credential functions ────────────────────────────────────────────

pub fn hash_password(password: &str) -> Result<String> {
    rustok_auth::hash_password(password).map_err(auth_err)
}

pub async fn hash_password_async(password: String) -> Result<String> {
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|err| Error::Message(format!("hash_password_async spawn_blocking: {err}")))?
}

pub fn verify_password(password: &str, password_hash: &str) -> Result<bool> {
    rustok_auth::verify_password(password, password_hash).map_err(auth_err)
}

pub async fn verify_password_async(password: String, password_hash: String) -> Result<bool> {
    tokio::task::spawn_blocking(move || verify_password(&password, &password_hash))
        .await
        .map_err(|err| Error::Message(format!("verify_password_async spawn_blocking: {err}")))?
}

pub fn generate_refresh_token() -> Result<String> {
    rustok_auth::generate_refresh_token().map_err(auth_err)
}

pub fn hash_refresh_token(token: &str) -> String {
    rustok_auth::hash_refresh_token(token)
}

const DUMMY_PASSWORD_VALUE: &str = "rustok-timing-equalization-dummy";

static DUMMY_PASSWORD_HASH: once_cell::sync::OnceCell<String> =
    once_cell::sync::OnceCell::new();

/// A valid Argon2id hash of a non-credential constant, generated once per
/// process. Verifying a candidate password against it costs the same as
/// verifying against a real stored hash, which keeps unknown-email logins
/// indistinguishable by latency from wrong-password logins.
pub fn dummy_password_hash() -> Result<String> {
    DUMMY_PASSWORD_HASH
        .get_or_try_init(|| {
            rustok_auth::hash_password(DUMMY_PASSWORD_VALUE).map_err(auth_err)
        })
        .cloned()
}

/// Async callers must not perform the one-time Argon2 hash generation on a
/// Tokio worker. The blocking pool also serializes initialization through the
/// shared OnceCell when several first-time requests arrive concurrently.
pub async fn dummy_password_hash_async() -> Result<String> {
    if let Some(hash) = DUMMY_PASSWORD_HASH.get() {
        return Ok(hash.clone());
    }
    tokio::task::spawn_blocking(dummy_password_hash)
        .await
        .map_err(|error| Error::Message(format!("dummy_password_hash_async spawn_blocking: {error}")))?
}

/// Perform a fixed-cost password verification on public account-lookup
/// paths so an unknown address does not return materially faster than a
/// known address. The dummy hash is prewarmed during host initialization.
pub async fn mask_auth_lookup_timing() -> Result<()> {
    let dummy_hash = dummy_password_hash_async().await?;
    verify_password_async(DUMMY_PASSWORD_VALUE.to_string(), dummy_hash).await?;
    Ok(())
}

// ─── Error conversion ────────────────────────────────────────────────

/// Convert `AuthError` to the server error bridge.
pub fn auth_err(err: AuthError) -> Error {
    match err {
        AuthError::InvalidCredentials | AuthError::InvalidAccessToken => {
            Error::Unauthorized(err.to_string())
        }
        AuthError::InvalidResetToken
        | AuthError::InvalidVerificationToken
        | AuthError::InvalidInviteToken => Error::Unauthorized(err.to_string()),
        AuthError::Validation(message) => Error::Validation(message),
        AuthError::TokenEncodingFailed
        | AuthError::PasswordHashFailed
        | AuthError::RefreshTokenGenerationFailed => Error::Message(format!("auth error: {err}")),
        AuthError::Internal(ref message) => {
            Error::Message(format!("auth internal error: {message}"))
        }
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

    fn mac_root() -> String {
        "test-secret-key-for-unit-tests-only-32bytes!".to_string()
    }

    #[tokio::test]
    async fn public_lookup_timing_mask_reuses_the_dummy_password_hash() {
        let first = dummy_password_hash_async().await.expect("dummy hash");
        let second = dummy_password_hash_async().await.expect("cached dummy hash");
        assert_eq!(first, second);
        mask_auth_lookup_timing()
            .await
            .expect("dummy verification should complete");
    }

    #[test]
    fn auth_config_from_complete_rsa_document_preserves_host_policy() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let (generation, config) = auth_config_from_host_document(
            mac_root(), 900, None, fixture.document(7).to_string().as_bytes(),
        ).expect("auth config");

        assert_eq!(generation, 7);
        assert_eq!(config.access_expiration, 900);
        assert_eq!(config.refresh_expiration, 60 * 60 * 24 * 30);
    }

    #[test]
    fn auth_config_rejects_malformed_nested_auth_settings() {
        let settings = serde_json::json!({
            "auth": {
                "algorithm": "RS999"
            }
        });
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let document = fixture.document(1).to_string();

        let result = auth_config_from_host_document(
            mac_root(), 900, Some(&settings), document.as_bytes(),
        );

        assert!(matches!(result, Err(crate::error::Error::Json(_))));
    }

    #[test]
    fn auth_config_rejects_startup_algorithm_overrides_with_file_source() {
        let settings = serde_json::json!({
            "auth": {
                "algorithm": "RS256"
            }
        });
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let document = fixture.document(1).to_string();

        let result = auth_config_from_host_document(
            mac_root(), 900, Some(&settings), document.as_bytes(),
        );

        assert!(result.is_err());
    }

    #[test]
    fn dummy_login_hash_is_valid_and_reuses_one_initialized_value() {
        let first = dummy_password_hash().expect("dummy hash");
        let second = dummy_password_hash().expect("cached dummy hash");
        assert_eq!(first, second);
        assert!(
            verify_password("rustok-timing-equalization-dummy", &first)
                .expect("dummy hash should be verifiable")
        );
    }

    fn rsa_test_config(root: String, document: &serde_json::Value) -> AuthConfig {
        auth_config_from_host_document(root, 900, None, document.to_string().as_bytes())
            .expect("complete RSA test keyring").1
    }

    #[test]
    fn password_reset_token_is_bound_to_credential_state() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let config = rsa_test_config(mac_root(), &fixture.document(1));
        let tenant_id = uuid::Uuid::new_v4();
        let token = encode_password_reset_token(
            &config,
            tenant_id,
            " User@Example.com ",
            "old-password-hash",
            900,
        )
        .expect("encode reset token");
        let claims = decode_password_reset_token(&config, &token).expect("decode reset token");

        assert_eq!(claims.sub, "user@example.com");
        assert_eq!(claims.tenant_id, tenant_id);
        assert!(is_canonical_auth_mac_tag(&claims.credential_fingerprint));
        assert!(password_reset_credential_matches(
            &config,
            "old-password-hash",
            &claims.credential_fingerprint,
        ));
        assert!(!password_reset_credential_matches(
            &config,
            "new-password-hash",
            &claims.credential_fingerprint,
        ));
    }

    #[test]
    fn reset_binding_rejects_legacy_and_different_secrets() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let document = fixture.document(1);
        let config = rsa_test_config(mac_root(), &document);
        let tag = password_reset_credential_fingerprint(&config, "hash").unwrap();
        assert!(password_reset_credential_matches(&config, "hash", &tag));
        assert!(!password_reset_credential_matches(&config, "hash", &format!("v2:{tag}")));
        assert!(!password_reset_credential_matches(&config, "hash", &tag[2..]));
        let tenant = uuid::Uuid::new_v4();
        for invalid in [format!("v2:{tag}"), tag.to_uppercase(), tag[2..].to_string()] {
            let subject = format!("user@example.com{TOKEN_SUBJECT_SEPARATOR}{invalid}");
            let token = rustok_auth::encode_password_reset_token(&config, tenant, &subject, 900).unwrap();
            assert!(decode_password_reset_token(&config, &token).is_err());
        }
        let other = rsa_test_config("another-secret-at-least-thirty-two-bytes".to_string(), &document);
        assert!(!password_reset_credential_matches(&other, "hash", &tag));
        assert!(
            auth_config_from_host_document(
                "short".to_string(), 900, None, document.to_string().as_bytes(),
            ).is_err(),
            "a weak MAC root must fail during Auth preparation",
        );
    }

    #[test]
    fn email_verification_token_is_bound_to_user_id() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let config = rsa_test_config(mac_root(), &fixture.document(1));
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let token =
            encode_email_verification_token(&config, tenant_id, user_id, " User@Example.com ", 900)
                .expect("encode verification token");
        let claims =
            decode_email_verification_token(&config, &token).expect("decode verification token");

        assert_eq!(claims.tenant_id, tenant_id);
        assert_eq!(claims.user_id, user_id);
        assert_eq!(claims.sub, "user@example.com");
    }
}
