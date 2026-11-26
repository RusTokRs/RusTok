use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, Validation, decode, encode};
use rustok_api::AuthPrincipalKind;
use rustok_core::UserRole;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[cfg(test)]
use jsonwebtoken::{EncodingKey, decode_header};
use uuid::Uuid;

use crate::config::AuthConfig;
use crate::error::{AuthError, Result};
use crate::jwt_keyring::{JwtPurpose, ValidatedJwtKeyring};

const DIRECT_GRANT_TYPE: &str = "direct";
const PASSWORD_RESET_PURPOSE: &str = "password_reset";
const EMAIL_VERIFICATION_PURPOSE: &str = "email_verification";
const INVITE_PURPOSE: &str = "invite";

// ─── Claims ──────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: Uuid,
    pub tenant_id: Uuid,
    pub role: UserRole,
    pub session_id: Uuid,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,

    // Principal-specific OAuth fields. Grant type and scopes are explicit.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<Uuid>,
    pub scopes: Vec<String>,
    pub grant_type: String,
    /// Registered OAuth access-token id (`jti`). Direct first-party
    /// session tokens leave this absent; delegated/service OAuth tokens
    /// must carry the id of their durable `oauth_tokens` access row so
    /// revocation can fence an already-issued JWT before its TTL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<Uuid>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteClaims {
    pub sub: String,
    pub tenant_id: Uuid,
    pub role: UserRole,
    /// Tenant-defined custom role layered over the baseline `role`
    /// membership at acceptance time. Absent for a baseline-role invite.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_role_slug: Option<String>,
    /// Mandatory unique issuance nonce. The persisted exact token digest
    /// remains the single-use authority.
    pub jti: Uuid,
    pub purpose: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PasswordResetClaims {
    pub sub: String,
    pub tenant_id: Uuid,
    pub purpose: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct EmailVerificationClaims {
    pub sub: String,
    pub tenant_id: Uuid,
    pub purpose: String,
    pub iss: String,
    pub aud: String,
    pub exp: usize,
    pub iat: usize,
}

// ─── Encode / Decode ─────────────────────────────────────────────────

pub fn encode_access_token(
    config: &AuthConfig,
    user_id: Uuid,
    tenant_id: Uuid,
    role: UserRole,
    session_id: Uuid,
) -> Result<String> {
    Ok(issue_direct_access_token(config, user_id, tenant_id, role, session_id, None)?.token)
}

/// Exact signed direct-access deadline, optionally bounded by the session.
pub fn issue_direct_access_token(
    config: &AuthConfig,
    user_id: Uuid,
    tenant_id: Uuid,
    role: UserRole,
    session_id: Uuid,
    deadline: Option<chrono::DateTime<Utc>>,
) -> Result<IssuedAccessToken> {
    if user_id.is_nil() || tenant_id.is_nil() || session_id.is_nil() {
        return Err(AuthError::TokenEncodingFailed);
    }
    tracing::debug!(
        user_id = %user_id,
        tenant_id = %tenant_id,
        role = ?role,
        session_id = %session_id,
        "Encoding access token"
    );
    let now = config.credential_now()?;
    let configured_expiry = token_expiration(now, config.access_expiration)?;
    let bounded_expiry = deadline.map_or(configured_expiry, |cap| configured_expiry.min(cap));
    let expires_at = chrono::DateTime::<Utc>::from_timestamp(bounded_expiry.timestamp(), 0)
        .ok_or(AuthError::TokenEncodingFailed)?;
    if now.timestamp() <= 0 || expires_at <= now {
        return Err(AuthError::TokenEncodingFailed);
    }

    let claims = Claims {
        sub: user_id,
        tenant_id,
        role,
        session_id,
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: usize::try_from(expires_at.timestamp()).map_err(|_| AuthError::TokenEncodingFailed)?,
        iat: usize::try_from(now.timestamp()).map_err(|_| AuthError::TokenEncodingFailed)?,
        client_id: None,
        scopes: Vec::new(),
        grant_type: DIRECT_GRANT_TYPE.to_string(),
        jti: None,
    };

    let token = encode_for_purpose(config, JwtPurpose::Access, &claims)?;
    let after = config.credential_now()?;
    if after < now || after >= expires_at {
        return Err(AuthError::TokenEncodingFailed);
    }
    Ok(IssuedAccessToken { token, issued_at: now, expires_at })
}

pub fn encode_oauth_access_token(
    config: &AuthConfig,
    input: OauthAccessTokenInput<'_>,
) -> Result<String> {
    Ok(issue_oauth_access_token(config, input, None)?.token)
}

/// The exact signed deadline accompanies the bearer for durable registration.
/// Do not derive a second expiry from an independent clock observation.
pub struct IssuedAccessToken {
    pub token: String,
    pub issued_at: chrono::DateTime<Utc>,
    pub expires_at: chrono::DateTime<Utc>,
}

pub type IssuedOauthAccessToken = IssuedAccessToken;

pub fn issue_oauth_access_token(
    config: &AuthConfig,
    input: OauthAccessTokenInput<'_>,
    deadline: Option<chrono::DateTime<Utc>>,
) -> Result<IssuedOauthAccessToken> {
    if input.app_id.is_nil()
        || input.tenant_id.is_nil()
        || input.client_id.is_nil()
        || input.token_id.is_nil()
    {
        return Err(AuthError::TokenEncodingFailed);
    }
    tracing::debug!(
        app_id = %input.app_id,
        tenant_id = %input.tenant_id,
        role = ?input.role,
        client_id = %input.client_id,
        grant_type = %input.grant_type,
        "Encoding OAuth access token"
    );
    let now = config.credential_now()?;
    let configured_expiry = token_expiration(now, input.expires_in_secs)?;
    let bounded_expiry = deadline.map_or(configured_expiry, |cap| configured_expiry.min(cap));
    // NumericDate has whole-second precision. Floor the absolute bound,
    // never round it up or derive another TTL after sampling the clock.
    let expires_at = chrono::DateTime::<Utc>::from_timestamp(bounded_expiry.timestamp(), 0)
        .ok_or(AuthError::TokenEncodingFailed)?;
    if now.timestamp() <= 0 || expires_at <= now {
        return Err(AuthError::TokenEncodingFailed);
    }

    let claims = Claims {
        sub: input.app_id,
        tenant_id: input.tenant_id,
        role: input.role,
        session_id: Uuid::nil(),
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: usize::try_from(expires_at.timestamp()).map_err(|_| AuthError::TokenEncodingFailed)?,
        iat: usize::try_from(now.timestamp()).map_err(|_| AuthError::TokenEncodingFailed)?,
        client_id: Some(input.client_id),
        scopes: input.scopes.to_vec(),
        grant_type: input.grant_type.to_string(),
        jti: Some(input.token_id),
    };

    if !valid_access_subject(&claims) {
        return Err(AuthError::TokenEncodingFailed);
    }
    let token = encode_for_purpose(config, JwtPurpose::Access, &claims)?;
    let after = config.credential_now()?;
    if after < now || after >= expires_at {
        return Err(AuthError::TokenEncodingFailed);
    }
    Ok(IssuedAccessToken { token, issued_at: now, expires_at })
}

pub struct OauthAccessTokenInput<'a> {
    pub app_id: Uuid,
    pub tenant_id: Uuid,
    pub role: UserRole,
    pub client_id: Uuid,
    pub scopes: &'a [String],
    pub grant_type: &'a str,
    pub expires_in_secs: u64,
    /// Durable access-token row id; becomes the JWT `jti`.
    pub token_id: Uuid,
}

pub fn decode_access_token(config: &AuthConfig, token: &str) -> Result<Claims> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AuthError::InvalidAccessToken);
    }
    tracing::debug!("Decoding access token");
    let claims = decode_for_purpose::<Claims>(config, token, JwtPurpose::Access)
        .map_err(|_| AuthError::InvalidAccessToken)?;

    if claims.sub.is_nil()
        || claims.tenant_id.is_nil()
        || !valid_access_subject(&claims)
        || !valid_token_times(claims.iat, claims.exp)
    {
        return Err(AuthError::InvalidAccessToken);
    }

    Ok(claims)
}

/// Keep JWT admission aligned with the platform's canonical principal
/// classification. The durable OAuth registration is still checked by the host.
fn valid_access_subject(claims: &Claims) -> bool {
    match AuthPrincipalKind::from_authenticated_facts(
        &claims.grant_type,
        claims.client_id,
        claims.session_id,
    ) {
        Some(AuthPrincipalKind::DirectUser) => claims.jti.is_none() && claims.scopes.is_empty(),
        Some(AuthPrincipalKind::DelegatedUser | AuthPrincipalKind::Service) => {
            claims.client_id.is_some_and(|id| !id.is_nil())
                && claims.jti.is_some_and(|id| !id.is_nil())
        }
        None => false,
    }
}

// ─── Special-purpose tokens ──────────────────────────────────────────

pub fn encode_password_reset_token(
    config: &AuthConfig,
    tenant_id: Uuid,
    email: &str,
    ttl_seconds: u64,
) -> Result<String> {
    let email = email.trim();
    if email.is_empty() || tenant_id.is_nil() {
        return Err(AuthError::TokenEncodingFailed);
    }
    tracing::debug!(tenant_id = %tenant_id, "Encoding password reset token");
    let now = Utc::now();
    let exp = token_expiration(now, ttl_seconds)?;

    let claims = PasswordResetClaims {
        sub: email.to_lowercase(),
        tenant_id,
        purpose: PASSWORD_RESET_PURPOSE.to_string(),
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode_for_purpose(config, JwtPurpose::PasswordReset, &claims)
}

pub fn decode_password_reset_token(
    config: &AuthConfig,
    token: &str,
) -> Result<PasswordResetClaims> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AuthError::InvalidResetToken);
    }
    tracing::debug!("Decoding password reset token");
    let claims = decode_for_purpose::<PasswordResetClaims>(config, token, JwtPurpose::PasswordReset)
        .map_err(|_| AuthError::InvalidResetToken)?;

    if claims.tenant_id.is_nil()
        || claims.sub.trim().is_empty()
        || !valid_token_times(claims.iat, claims.exp)
        || claims.purpose != PASSWORD_RESET_PURPOSE
    {
        return Err(AuthError::InvalidResetToken);
    }

    Ok(claims)
}

pub fn encode_email_verification_token(
    config: &AuthConfig,
    tenant_id: Uuid,
    email: &str,
    ttl_seconds: u64,
) -> Result<String> {
    let email = email.trim();
    if email.is_empty() || tenant_id.is_nil() {
        return Err(AuthError::TokenEncodingFailed);
    }
    tracing::debug!(tenant_id = %tenant_id, "Encoding email verification token");
    let now = Utc::now();
    let exp = token_expiration(now, ttl_seconds)?;

    let claims = EmailVerificationClaims {
        sub: email.to_lowercase(),
        tenant_id,
        purpose: EMAIL_VERIFICATION_PURPOSE.to_string(),
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode_for_purpose(config, JwtPurpose::EmailVerification, &claims)
}

pub fn decode_email_verification_token(
    config: &AuthConfig,
    token: &str,
) -> Result<EmailVerificationClaims> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AuthError::InvalidVerificationToken);
    }
    tracing::debug!("Decoding email verification token");
    let claims = decode_for_purpose::<EmailVerificationClaims>(config, token, JwtPurpose::EmailVerification)
        .map_err(|_| AuthError::InvalidVerificationToken)?;

    if claims.tenant_id.is_nil()
        || claims.sub.trim().is_empty()
        || !valid_token_times(claims.iat, claims.exp)
        || claims.purpose != EMAIL_VERIFICATION_PURPOSE
    {
        return Err(AuthError::InvalidVerificationToken);
    }

    Ok(claims)
}

pub fn encode_invite_token(
    config: &AuthConfig,
    tenant_id: Uuid,
    email: &str,
    role: UserRole,
    custom_role_slug: Option<String>,
    ttl_seconds: u64,
) -> Result<String> {
    let email = email.trim();
    if email.is_empty() || tenant_id.is_nil() {
        return Err(AuthError::TokenEncodingFailed);
    }
    if !valid_invite_role(&role, custom_role_slug.as_deref()) {
        return Err(AuthError::TokenEncodingFailed);
    }
    tracing::debug!(tenant_id = %tenant_id, role = ?role, "Encoding invite token");
    let now = Utc::now();
    let exp = token_expiration(now, ttl_seconds)?;

    let claims = InviteClaims {
        sub: email.to_lowercase(),
        tenant_id,
        role,
        custom_role_slug,
        jti: Uuid::new_v4(),
        purpose: INVITE_PURPOSE.to_string(),
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode_for_purpose(config, JwtPurpose::Invite, &claims)
}

pub fn decode_invite_token(config: &AuthConfig, token: &str) -> Result<InviteClaims> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AuthError::InvalidInviteToken);
    }
    tracing::debug!("Decoding invite token");
    let claims = decode_for_purpose::<InviteClaims>(config, token, JwtPurpose::Invite)
        .map_err(|_| AuthError::InvalidInviteToken)?;

    if claims.tenant_id.is_nil()
        || claims.sub.trim().is_empty()
        || !valid_token_times(claims.iat, claims.exp)
        || claims.purpose != INVITE_PURPOSE
        || claims.jti.is_nil()
    {
        return Err(AuthError::InvalidInviteToken);
    }
    if !valid_invite_role(&claims.role, claims.custom_role_slug.as_deref()) {
        return Err(AuthError::InvalidInviteToken);
    }

    Ok(claims)
}

fn valid_invite_role(role: &UserRole, custom_role_slug: Option<&str>) -> bool {
    // A custom role is layered over baseline customer membership.
    custom_role_slug.is_none_or(|slug| {
        !slug.trim().is_empty() && slug.len() <= 64 && *role == UserRole::Customer
    })
}

fn token_expiration(now: chrono::DateTime<Utc>, ttl_seconds: u64) -> Result<chrono::DateTime<Utc>> {
    if ttl_seconds == 0 {
        return Err(AuthError::TokenEncodingFailed);
    }
    let ttl_seconds = i64::try_from(ttl_seconds)
        .map_err(|_| AuthError::Internal("Token expiration is out of range".to_string()))?;

    let ttl = Duration::try_seconds(ttl_seconds)
        .ok_or_else(|| AuthError::Internal("Token expiration is out of range".to_string()))?;
    let exp = now.checked_add_signed(ttl)
        .ok_or_else(|| AuthError::Internal("Token expiration is out of range".to_string()))?;
    // Claims use usize on the wire; prevent truncation on narrow targets.
    usize::try_from(now.timestamp()).map_err(|_| AuthError::TokenEncodingFailed)?;
    usize::try_from(exp.timestamp()).map_err(|_| AuthError::TokenEncodingFailed)?;
    Ok(exp)
}

// ─── Key helpers ─────────────────────────────────────────────────────

fn prepared_header(ring: &ValidatedJwtKeyring, purpose: JwtPurpose) -> Header {
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(ring.active_kid().to_string());
    header.typ = Some(purpose.typ().to_string());
    header
}

fn encode_for_purpose<T: Serialize>(
    config: &AuthConfig, purpose: JwtPurpose, claims: &T,
) -> Result<String> {
    config.with_jwt_operation(|prepared| {
        let ring = prepared.for_purpose(purpose);
        encode(&prepared_header(ring, purpose), claims, ring.signing_key())
            .map_err(|_| AuthError::TokenEncodingFailed)
    })
}

#[cfg(test)]
fn jwt_header(config: &AuthConfig, purpose: JwtPurpose) -> Header {
    prepared_header(config.prepared_jwt().unwrap().for_purpose(purpose), purpose)
}

#[cfg(test)]
fn encoding_key(config: &AuthConfig, purpose: JwtPurpose) -> Result<&EncodingKey> {
    Ok(config.prepared_jwt()?.for_purpose(purpose).signing_key())
}

/// Only the JOSE fields emitted by Auth are supported; every purpose requires
/// a non-null, exact key identifier.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthJwtHeader {
    alg: Algorithm,
    typ: String,
    kid: String,
}

fn auth_jwt_header(token: &str) -> Result<AuthJwtHeader> {
    let (encoded, _) = token.split_once('.').ok_or(AuthError::InvalidAccessToken)?;
    // Every supported header has three small scalar fields. Bound work
    // before allocating or parsing attacker-controlled header JSON.
    if encoded.len() > 1024 {
        return Err(AuthError::InvalidAccessToken);
    }
    let bytes = URL_SAFE_NO_PAD.decode(encoded).map_err(|_| AuthError::InvalidAccessToken)?;
    serde_json::from_slice(&bytes).map_err(|_| AuthError::InvalidAccessToken)
}

/// The untrusted header selects exactly one key in the expected ring.
/// Recheck the exclusive retirement deadline after signature verification.
fn decode_for_purpose<T: DeserializeOwned>(
    config: &AuthConfig,
    token: &str,
    purpose: JwtPurpose,
) -> Result<T> {
    let header = auth_jwt_header(token)?;
    config.with_jwt_operation(|prepared| {
        let selected = prepared.for_purpose(purpose);
        if header.typ != purpose.typ() || header.alg != Algorithm::RS256 {
            return Err(AuthError::InvalidAccessToken);
        }
        let kid = header.kid.as_str();
        let (key, cutoff) = selected.select(kid, Utc::now().timestamp())
            .ok_or(AuthError::InvalidAccessToken)?;
        let validation = strict_jwt_validation(config);
        let claims = decode::<T>(token, key, &validation)
            .map_err(|_| AuthError::InvalidAccessToken)?.claims;
        if cutoff.is_some_and(|until| Utc::now().timestamp() >= until) {
            return Err(AuthError::InvalidAccessToken);
        }
        Ok(claims)
    })
}

// ─── Validation ──────────────────────────────────────────────────────

fn valid_token_times(iat: usize, exp: usize) -> bool {
    let (Ok(iat), Ok(exp)) = (i64::try_from(iat), i64::try_from(exp)) else {
        return false;
    };
    let Some(expiration) = chrono::DateTime::<Utc>::from_timestamp(exp, 0) else {
        return false;
    };
    let now = Utc::now().timestamp();
    iat > 0 && iat <= now && iat < expiration.timestamp() && now < expiration.timestamp()
}

fn strict_jwt_validation(config: &AuthConfig) -> Validation {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_required_spec_claims(&["exp", "iss", "aud", "sub"]);
    validation.validate_exp = true;
    validation.validate_nbf = true;
    // Strict expiration: the same authority issues and validates these
    // tokens, so there is no cross-machine clock skew to absorb.
    validation.leeway = 0;
    // RFC 7519: token MUST NOT be accepted on or after `exp`.
    validation.reject_tokens_expiring_in_less_than = 1;
    validation.set_issuer(&[config.issuer.as_str()]);
    validation.set_audience(&[config.audience.as_str()]);
    validation
}

// ─── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_test_utils::jwt::{JwtKeyringFixture, RsaKeyPairFixture};

    const ROOT: &str = "test-secret-key-for-unit-tests-only-32bytes!";

    fn config_from_document(doc: &serde_json::Value) -> AuthConfig {
        crate::build_auth_config_from_document(
            ROOT.to_string(), 900, Default::default(), doc.to_string().as_bytes(),
        ).unwrap().1
    }

    fn test_config() -> AuthConfig {
        config_from_document(&JwtKeyringFixture::generate().unwrap().document(1))
    }


    #[test]
    fn jwt_keyring_rotation_selects_one_key_and_enforces_retirement() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let next = RsaKeyPairFixture::generate().unwrap();
        let old = config_from_document(&fixture.document(1));
        let tenant = Uuid::new_v4();
        let user = Uuid::new_v4();
        let session = Uuid::new_v4();
        let token = encode_access_token(&old, user, tenant, UserRole::Customer, session).unwrap();
        let old_claims = decode_access_token(&old, &token).unwrap();
        let oauth = encode_oauth_access_token(&old, OauthAccessTokenInput {
            app_id: user,
            tenant_id: tenant,
            role: UserRole::Customer,
            client_id: Uuid::new_v4(),
            scopes: &["read".to_string()],
            grant_type: "client_credentials",
            expires_in_secs: 900,
            token_id: Uuid::new_v4(),
        }).unwrap();
        let mut no_kid = jwt_header(&old, JwtPurpose::Access);
        no_kid.kid = None;
        let legacy = encode(&no_kid, &old_claims, encoding_key(&old, JwtPurpose::Access).unwrap()).unwrap();
        assert!(decode_access_token(&old, &legacy).is_err());
        let now = Utc::now().timestamp();
        let settings = |until| fixture.document_with_access(
            2, "next", &next, Some(("access", &fixture.access, until)),
        );
        let rotated = config_from_document(&settings(now + 3600));
        assert_eq!(decode_access_token(&rotated, &token).unwrap().sub, user);
        assert_eq!(decode_access_token(&rotated, &oauth).unwrap().grant_type, "client_credentials");
        assert!(decode_access_token(&rotated, &legacy).is_err());
        let fresh = encode_access_token(&rotated, user, tenant, UserRole::Customer, session).unwrap();
        assert_eq!(decode_header(&fresh).unwrap().kid.as_deref(), Some("next"));
        assert!(decode_access_token(&rotated, &fresh).is_ok());
        assert!(decode_access_token(&old, &fresh).is_err());
        for kid in ["unknown", "", "access"] {
            let mut header = jwt_header(&rotated, JwtPurpose::Access);
            header.kid = Some(kid.to_string());
            let forged = encode(&header, &old_claims, encoding_key(&rotated, JwtPurpose::Access).unwrap()).unwrap();
            assert!(decode_access_token(&rotated, &forged).is_err());
        }
        let retired = config_from_document(&settings(now - 1));
        assert!(decode_access_token(&retired, &token).is_err());
        assert!(decode_access_token(&retired, &legacy).is_err());
        assert!(decode_access_token(&retired, &fresh).is_ok());
    }

    #[test]
    fn purpose_rotation_is_independent_and_rejects_cross_signed_claims() {
        use serde_json::json;
        let fixture = JwtKeyringFixture::generate().unwrap();
        let next = RsaKeyPairFixture::generate().unwrap();
        let old = config_from_document(&fixture.document(1));
        let tenant = Uuid::new_v4();
        let reset = encode_password_reset_token(&old, tenant, "a@example.com", 900).unwrap();
        let verify = encode_email_verification_token(&old, tenant, "a@example.com", 900).unwrap();
        let invite = encode_invite_token(&old, tenant, "a@example.com", UserRole::Customer, None, 900).unwrap();
        let access = encode_access_token(&old, Uuid::new_v4(), tenant, UserRole::Customer, Uuid::new_v4()).unwrap();
        let reset_claims = decode_password_reset_token(&old, &reset).unwrap();
        let access_claims = decode_access_token(&old, &access).unwrap();
        let settings = |until| {
            let mut doc = fixture.document(2);
            let mut ring = next.ring("reset-next");
            ring["keyring"]["verification_keys"] = json!([{
                "kid": "password-reset", "material": fixture.password_reset.public_pem(),
                "verify_until": until,
            }]);
            doc["password_reset"] = ring;
            doc
        };
        let now = Utc::now().timestamp();
        let rotated = config_from_document(&settings(now + 3600));
        assert!(decode_password_reset_token(&rotated, &reset).is_ok());
        assert!(decode_email_verification_token(&rotated, &verify).is_ok());
        assert!(decode_invite_token(&rotated, &invite).is_ok());
        assert!(decode_access_token(&rotated, &access).is_ok());

        let fresh = encode_password_reset_token(&rotated, tenant, "a@example.com", 900).unwrap();
        assert_eq!(decode_header(&fresh).unwrap().kid.as_deref(), Some("reset-next"));
        assert!(decode_password_reset_token(&old, &fresh).is_err());
        // A forged reset with the correct reset header but an access signature must fail.
        let forged_reset = encode(
            &jwt_header(&rotated, JwtPurpose::PasswordReset),
            &reset_claims,
            encoding_key(&rotated, JwtPurpose::Access).unwrap(),
        ).unwrap();
        assert!(decode_password_reset_token(&rotated, &forged_reset).is_err());
        // A forged access token with the correct access header but a reset signature must fail.
        let forged_access = encode(
            &jwt_header(&rotated, JwtPurpose::Access),
            &access_claims,
            encoding_key(&rotated, JwtPurpose::PasswordReset).unwrap(),
        ).unwrap();
        assert!(decode_access_token(&rotated, &forged_access).is_err());
        // Even a valid reset signature cannot override the expected typ.
        let mut header = jwt_header(&rotated, JwtPurpose::PasswordReset);
        header.typ = Some("JWT".to_string());
        let wrong_typ = encode(&header, &reset_claims,
            encoding_key(&rotated, JwtPurpose::PasswordReset).unwrap()).unwrap();
        assert!(decode_password_reset_token(&rotated, &wrong_typ).is_err());
        header.typ = Some(JwtPurpose::PasswordReset.typ().to_string());
        header.kid = None;
        let missing_kid = encode(&header, &reset_claims,
            encoding_key(&rotated, JwtPurpose::PasswordReset).unwrap()).unwrap();
        assert!(decode_password_reset_token(&rotated, &missing_kid).is_err());
        let rotated = config_from_document(&settings(now - 1));
        assert!(decode_password_reset_token(&rotated, &reset).is_err());
        assert!(decode_password_reset_token(&rotated, &fresh).is_ok());
        assert!(decode_email_verification_token(&rotated, &verify).is_ok());
        assert!(decode_invite_token(&rotated, &invite).is_ok());
        assert!(decode_access_token(&rotated, &access).is_ok());
    }

    #[test]
    fn non_rs256_jose_header_and_raw_root_history_are_rejected() {
        use serde_json::json;
        let old = test_config();
        let tenant = Uuid::new_v4();
        let access = encode_access_token(&old, Uuid::new_v4(), tenant, UserRole::Customer, Uuid::new_v4()).unwrap();
        let mut segments = access.split('.');
        let _original_header = segments.next().unwrap();
        let payload = segments.next().unwrap();
        let signature = segments.next().unwrap();
        assert!(segments.next().is_none());
        let forged_header = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&json!({
            "alg": "HS256", "typ": JwtPurpose::Access.typ(), "kid": "access"
        })).unwrap());
        let forged = format!("{forged_header}.{payload}.{signature}");
        assert!(decode_access_token(&old, &forged).is_err());

        let fixture = JwtKeyringFixture::generate().unwrap();
        let mut document = fixture.document(2);
        document["access"]["keyring"]["verification_keys"] = json!([{
            "kid": "mac-root", "material": old.secret(),
            "verify_until": Utc::now().timestamp() + 3600
        }]);
        assert!(crate::build_auth_config_from_document(
            old.secret().to_string(), 900, Default::default(), document.to_string().as_bytes(),
        ).is_err());
    }

    #[test]
    fn direct_configuration_cannot_bypass_cross_ring_key_isolation() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let mut doc = fixture.document(1);
        let tenant = Uuid::new_v4();
        let config = config_from_document(&doc);
        let token = encode_access_token(&config, Uuid::new_v4(), tenant,
            UserRole::Customer, Uuid::new_v4()).unwrap();
        doc["invite"] = fixture.access.ring("invite-next");
        let invalid = AuthConfig::from_test_document_unvalidated(ROOT.into(),
            doc.to_string().as_bytes());
        assert!(decode_access_token(&invalid, &token).is_err());
        assert!(encode_invite_token(&invalid, tenant, "a@example.com",
            UserRole::Customer, None, 900).is_err());
    }

    #[test]
    fn rs256_rotation_stages_promotes_and_retires_distinct_keys() {
        let now = Utc::now().timestamp();
        let user = Uuid::new_v4();
        let tenant = Uuid::new_v4();
        let session = Uuid::new_v4();
        let fixture = JwtKeyringFixture::generate().unwrap();
        let next = RsaKeyPairFixture::generate().unwrap();
        let old = config_from_document(&fixture.document(1));
        let issue = |config: &AuthConfig| {
            encode_access_token(config, user, tenant, UserRole::Customer, session).unwrap()
        };
        let old_token = issue(&old);
        let old_oauth = encode_oauth_access_token(&old, OauthAccessTokenInput {
            app_id: user,
            tenant_id: tenant,
            role: UserRole::Customer,
            client_id: Uuid::new_v4(),
            scopes: &["catalog:read".to_string()],
            grant_type: "client_credentials",
            expires_in_secs: 900,
            token_id: Uuid::new_v4(),
        }).unwrap();
        let reset = encode_password_reset_token(&old, tenant, "a@example.com", 900).unwrap();

        let staged = config_from_document(&fixture.document_with_access(
            2, "access", &fixture.access, Some(("rsa-next", &next, now + 3600)),
        ));
        assert_eq!(decode_header(&issue(&staged)).unwrap().kid.as_deref(), Some("access"));

        let history = |until| fixture.document_with_access(
            3, "rsa-next", &next, Some(("access", &fixture.access, until)),
        );
        let promoted = config_from_document(&history(now + 3600));
        let fresh = issue(&promoted);
        let header = decode_header(&fresh).unwrap();
        assert_eq!(header.alg, Algorithm::RS256);
        assert_eq!(header.kid.as_deref(), Some("rsa-next"));
        assert!(decode_access_token(&staged, &fresh).is_ok());
        assert!(decode_access_token(&old, &fresh).is_err());
        assert!(decode_access_token(&promoted, &old_token).is_ok());
        assert!(decode_access_token(&promoted, &old_oauth).is_ok());

        let claims = decode_access_token(&old, &old_token).unwrap();
        let mut legacy_header = jwt_header(&old, JwtPurpose::Access);
        legacy_header.kid = None;
        let legacy = encode(&legacy_header, &claims, encoding_key(&old, JwtPurpose::Access).unwrap()).unwrap();
        assert!(decode_access_token(&promoted, &legacy).is_err());
        for wrong_id in ["rsa-next", "unknown"] {
            let mut wrong_header = jwt_header(&old, JwtPurpose::Access);
            wrong_header.kid = Some(wrong_id.to_string());
            let wrong = encode(&wrong_header, &claims, encoding_key(&old, JwtPurpose::Access).unwrap()).unwrap();
            assert!(decode_access_token(&promoted, &wrong).is_err());
        }
        assert!(decode_password_reset_token(&promoted, &reset).is_ok());

        let retired = config_from_document(&history(now - 1));
        assert!(decode_access_token(&retired, &old_token).is_err());
        assert!(decode_access_token(&retired, &old_oauth).is_err());
        assert!(decode_access_token(&retired, &legacy).is_err());
        assert!(decode_access_token(&retired, &fresh).is_ok());
        assert!(decode_password_reset_token(&retired, &reset).is_ok());
        // Immutable snapshots continue to behave according to their own cutoffs.
        assert!(decode_access_token(&promoted, &old_token).is_ok());
    }

    #[test]
    fn independent_rsa_special_ring_does_not_change_other_purposes() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let next = RsaKeyPairFixture::generate().unwrap();
        let original = config_from_document(&fixture.document(1));
        let tenant = Uuid::new_v4();
        let access = encode_access_token(&original, Uuid::new_v4(), tenant,
            UserRole::Customer, Uuid::new_v4()).unwrap();
        let mut doc = fixture.document(2);
        doc["invite"] = next.ring("invite-rsa");
        let config = config_from_document(&doc);
        let invite = encode_invite_token(&config, tenant, "a@example.com",
            UserRole::Customer, None, 900).unwrap();
        assert_eq!(decode_header(&invite).unwrap().alg, Algorithm::RS256);
        assert!(decode_invite_token(&config, &invite).is_ok());
        assert!(decode_access_token(&config, &access).is_ok());
        assert!(decode_invite_token(&original, &invite).is_err());
        doc["access"] = next.ring("duplicate");
        assert!(crate::build_auth_config_from_document(
            ROOT.into(), 900, Default::default(), doc.to_string().as_bytes(),
        ).is_err());
    }

    #[test]
    fn weak_mac_root_fails_closed_at_key_materialization() {
        // AuthConfig can be built without build_auth_config; signing or
        // verifying with a sub-32-byte secret must still be impossible.
        let fixture = JwtKeyringFixture::generate().unwrap();
        let doc = fixture.document(1);
        let weak = AuthConfig::from_test_document_unvalidated(
            "short-secret".into(), doc.to_string().as_bytes(),
        );
        let encode = encode_access_token(
            &weak,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        );
        assert!(encode.is_err());

        let strong = config_from_document(&doc);
        let token = encode_access_token(
            &strong,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        )
        .unwrap();
        assert!(decode_access_token(&weak, &token).is_err());
    }

    #[test]
    fn whitespace_padded_mac_root_fails_closed() {
        // 31 real bytes with an interior space: `trim().len()` counts 32 and
        // would pass, but whitespace must not credit toward the minimum.
        let fixture = JwtKeyringFixture::generate().unwrap();
        let padded = AuthConfig::from_test_document_unvalidated(
            format!("{} {}", "a".repeat(16), "b".repeat(15)),
            fixture.document(1).to_string().as_bytes(),
        );
        let encode = encode_access_token(
            &padded,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        );
        assert!(encode.is_err());
        assert!(!crate::config::symmetric_secret_has_min_length(padded.secret()));
        assert!(crate::config::symmetric_secret_has_min_length(
            &"a".repeat(32)
        ));
    }

    #[test]
    fn rfc7519_jwt_required_claims_present() {
        let config = test_config();
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        )
        .unwrap();

        let claims = decode_access_token(&config, &token).unwrap();
        assert_ne!(claims.sub, Uuid::nil());
        assert_eq!(claims.iss, "rustok");
        assert_eq!(claims.aud, "rustok-admin");
        assert!(claims.exp > claims.iat);
        assert!(claims.iat > 0);
    }

    #[test]
    fn access_token_header_has_exact_purpose_and_kid() {
        let config = test_config();
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        )
        .unwrap();

        let header = jsonwebtoken::decode_header(&token).unwrap();
        assert_eq!(header.alg, Algorithm::RS256);
        assert_eq!(header.typ.as_deref(), Some(JwtPurpose::Access.typ()));
        assert_eq!(header.kid.as_deref(), Some("access"));
    }

    #[test]
    fn rs256_access_token_round_trips_and_header_matches_config() {
        let config = test_config();
        let user_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let token =
            encode_access_token(&config, user_id, tenant_id, UserRole::Admin, session_id).unwrap();

        let header = jsonwebtoken::decode_header(&token).unwrap();
        assert_eq!(header.alg, Algorithm::RS256);

        let claims = decode_access_token(&config, &token).unwrap();
        assert_eq!(claims.sub, user_id);
        assert_eq!(claims.tenant_id, tenant_id);
        assert_eq!(claims.session_id, session_id);
    }

    #[test]
    fn rs256_special_purposes_round_trip() {
        let config = test_config();
        let tenant_id = Uuid::new_v4();

        let password_token =
            encode_password_reset_token(&config, tenant_id, "USER@example.com", 900).unwrap();
        assert_eq!(decode_header(&password_token).unwrap().alg, Algorithm::RS256);
        let password_claims = decode_password_reset_token(&config, &password_token).unwrap();
        assert_eq!(password_claims.sub, "user@example.com");

        let verification_token =
            encode_email_verification_token(&config, tenant_id, "USER@example.com", 900).unwrap();
        assert_eq!(decode_header(&verification_token).unwrap().alg, Algorithm::RS256);
        let verification_claims =
            decode_email_verification_token(&config, &verification_token).unwrap();
        assert_eq!(verification_claims.sub, "user@example.com");

        let invite_token = encode_invite_token(
            &config,
            tenant_id,
            "INVITED@example.com",
            UserRole::Admin,
            None,
            900,
        )
        .unwrap();
        assert_eq!(decode_header(&invite_token).unwrap().alg, Algorithm::RS256);
        let invite_claims = decode_invite_token(&config, &invite_token).unwrap();
        assert_eq!(invite_claims.sub, "invited@example.com");
        assert_eq!(invite_claims.role, UserRole::Admin);
    }

    #[test]
    fn oauth_issuance_metadata_matches_signed_numeric_dates_and_absolute_cap() {
        let config = test_config();
        let scopes = vec!["read".to_string()];
        let input = |grant_type: &'static str| OauthAccessTokenInput {
            app_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            role: UserRole::Customer,
            client_id: Uuid::new_v4(),
            scopes: &scopes,
            grant_type,
            expires_in_secs: 900,
            token_id: Uuid::new_v4(),
        };
        let cap = chrono::DateTime::<Utc>::from_timestamp(Utc::now().timestamp() + 60, 123_000_000).unwrap();
        let issued = issue_oauth_access_token(&config, input("authorization_code"), Some(cap)).unwrap();
        let claims = decode_access_token(&config, &issued.token).unwrap();
        assert_eq!(claims.iat, issued.issued_at.timestamp() as usize);
        assert_eq!(claims.exp, issued.expires_at.timestamp() as usize);
        assert_eq!(issued.expires_at.timestamp_subsec_nanos(), 0);
        assert_eq!(issued.expires_at.timestamp(), cap.timestamp());
        assert!(issued.expires_at <= cap);
        assert!(issue_oauth_access_token(&config, input("authorization_code"), Some(Utc::now())).is_err());
        let uncapped = issue_oauth_access_token(&config, input("client_credentials"), None).unwrap();
        assert_eq!(
            uncapped.expires_at.timestamp() - uncapped.issued_at.timestamp(),
            900,
        );
    }

    #[test]
    fn token_encoders_reject_ttl_values_that_cannot_be_represented_safely() {
        let config = test_config();

        // These fit i64 but exceed chrono's representable TimeDelta.
        for ttl in [i64::MAX as u64, (i64::MAX / 2) as u64] {
            assert!(token_expiration(Utc::now(), ttl).is_err());
        }
        let access = config.clone().with_expiration(u64::MAX, config.refresh_expiration);
        assert!(
            encode_access_token(
                &access,
                Uuid::new_v4(),
                Uuid::new_v4(),
                UserRole::Customer,
                Uuid::new_v4(),
            )
            .is_err()
        );

        assert!(
            encode_oauth_access_token(
                &config,
                OauthAccessTokenInput {
                    app_id: Uuid::new_v4(),
                    tenant_id: Uuid::new_v4(),
                    role: UserRole::Customer,
                    client_id: Uuid::new_v4(),
                    scopes: &[],
                    grant_type: "client_credentials",
                    expires_in_secs: u64::MAX,
                    token_id: Uuid::new_v4(),
                },
            )
            .is_err()
        );

        assert!(
            encode_password_reset_token(&config, Uuid::new_v4(), "user@example.com", u64::MAX)
                .is_err()
        );
        assert!(
            encode_email_verification_token(&config, Uuid::new_v4(), "user@example.com", u64::MAX)
                .is_err()
        );
        assert!(
            encode_invite_token(
                &config,
                Uuid::new_v4(),
                "user@example.com",
                UserRole::Customer,
                None,
                u64::MAX
            )
            .is_err()
        );
    }

    #[test]
    fn signed_tokens_with_invalid_time_claims_are_rejected() {
        let config = test_config();
        let now = Utc::now().timestamp() as usize;
        let mut claims = Claims {
            sub: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            role: UserRole::Customer,
            session_id: Uuid::new_v4(),
            iss: config.issuer.clone(),
            aud: config.audience.clone(),
            exp: now + 600,
            iat: now + 60,
            client_id: None,
            scopes: vec![],
            grant_type: DIRECT_GRANT_TYPE.to_string(),
            jti: None,
        };
        for issued_at in [now + 60, claims.exp, 0] {
            claims.iat = issued_at;
            let signed = encode(&jwt_header(&config, JwtPurpose::Access), &claims, encoding_key(&config, JwtPurpose::Access).unwrap())
                .unwrap();
            assert!(decode_access_token(&config, &signed).is_err());
        }

        claims.iat = now;
        let mut payload = serde_json::to_value(&claims).unwrap();
        payload["nbf"] = serde_json::json!(now + 60);
        let signed = encode(&jwt_header(&config, JwtPurpose::Access), &payload, encoding_key(&config, JwtPurpose::Access).unwrap())
            .unwrap();
        assert!(decode_access_token(&config, &signed).is_err());

        let reset = PasswordResetClaims {
            sub: "user@example.com".to_string(),
            tenant_id: claims.tenant_id,
            purpose: PASSWORD_RESET_PURPOSE.to_string(),
            iss: config.issuer.clone(),
            aud: config.audience.clone(),
            exp: now + 600,
            iat: now + 60,
        };
        let signed = encode(&jwt_header(&config, JwtPurpose::PasswordReset), &reset, encoding_key(&config, JwtPurpose::PasswordReset).unwrap())
            .unwrap();
        assert!(decode_password_reset_token(&config, &signed).is_err());
    }

    #[test]
    fn jwt_time_claims_reject_unsigned_overflow_and_unrepresentable_dates() {
        if let Ok(max_signed) = usize::try_from(i64::MAX) {
            assert!(!valid_token_times(1, max_signed));
            assert!(!valid_token_times(max_signed, max_signed));
            if let Some(overflow) = max_signed.checked_add(1) {
                assert!(!valid_token_times(1, overflow));
                assert!(!valid_token_times(overflow, overflow));
            }
        }
    }

    #[test]
    fn direct_access_deadline_is_the_exact_signed_session_bound() {
        let config = test_config();
        let cap = Utc::now() + Duration::seconds(30);
        let issued = issue_direct_access_token(
            &config, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer,
            Uuid::new_v4(), Some(cap),
        ).unwrap();
        let claims = decode_access_token(&config, &issued.token).unwrap();
        assert_eq!(claims.exp as i64, issued.expires_at.timestamp());
        assert!(issued.expires_at <= cap);
        assert_eq!(claims.iat as i64, issued.issued_at.timestamp());
    }

    #[test]
    fn rfc7519_jwt_expiration_enforced() {
        let config = test_config();
        let token = encode_access_token(&config, Uuid::new_v4(), Uuid::new_v4(),
            UserRole::Customer, Uuid::new_v4()).unwrap();
        let mut claims = decode_access_token(&config, &token).unwrap();
        let now = Utc::now().timestamp() as usize;
        claims.iat = now - 60;
        claims.exp = now;
        let expired = encode(&jwt_header(&config, JwtPurpose::Access), &claims,
            encoding_key(&config, JwtPurpose::Access).unwrap()).unwrap();
        assert!(decode_access_token(&config, &expired).is_err());
        let zero = config.with_expiration(0, 900);
        assert!(encode_access_token(&zero, Uuid::new_v4(), Uuid::new_v4(),
            UserRole::Customer, Uuid::new_v4()).is_err());
        assert!(encode_password_reset_token(&zero, Uuid::new_v4(), "a@example.com", 0).is_err());
        assert!(encode_email_verification_token(&zero, Uuid::new_v4(), "a@example.com", 0).is_err());
        assert!(encode_invite_token(&zero, Uuid::new_v4(), "a@example.com", UserRole::Customer, None, 0).is_err());
    }

    #[test]
    fn auth_header_rejects_null_duplicate_and_unsupported_fields() {
        for raw in [
            r#"{"alg":"RS256","typ":"JWT","kid":null}"#,
            r#"{"alg":"RS256","typ":"JWT","kid":"a","kid":"b"}"#,
            r#"{"alg":"RS256","typ":"JWT","kid":"a","crit":["b64"],"b64":false}"#,
            r#"{"alg":"RS256","typ":"JWT","kid":"a","jku":"https://example.invalid"}"#,
            r#"{"alg":"RS256","typ":"JWT","kid":"a","unknown":1}"#,
            r#"{"alg":"RS256","kid":"a"}"#,
        ] {
            let token = format!("{}.e30.signature", URL_SAFE_NO_PAD.encode(raw));
            assert!(auth_jwt_header(&token).is_err());
        }
        let omitted = format!("{}.e30.signature", URL_SAFE_NO_PAD.encode(r#"{"alg":"RS256","typ":"JWT"}"#));
        assert!(auth_jwt_header(&omitted).is_err());
        let valid = format!("{}.e30.signature", URL_SAFE_NO_PAD.encode(
            r#"{"alg":"RS256","typ":"JWT","kid":"a"}"#,
        ));
        let parsed = auth_jwt_header(&valid).unwrap();
        assert_eq!(parsed.kid, "a");
        assert_eq!(parsed.alg, Algorithm::RS256);
        assert_eq!(parsed.typ, "JWT");
        let oversized = format!("{}.e30.signature", "a".repeat(1025));
        assert!(auth_jwt_header(&oversized).is_err());
    }

    #[test]
    fn rsa_direct_signer_rejects_mismatched_pair() {
        let fixture = JwtKeyringFixture::generate().unwrap();
        let wrong = RsaKeyPairFixture::generate().unwrap();
        let mut doc = fixture.document(1);
        doc["access"]["rsa_public_key_pem"] = serde_json::json!(wrong.public_pem());
        let config = AuthConfig::from_test_document_unvalidated(ROOT.into(),
            doc.to_string().as_bytes());
        assert!(encode_access_token(&config, Uuid::new_v4(), Uuid::new_v4(),
            UserRole::Customer, Uuid::new_v4()).is_err());
    }

    #[test]
    fn rfc7519_jwt_issuer_validated() {
        let config = test_config();
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        )
        .unwrap();

        let wrong_config = config.clone().with_issuer("wrong-issuer");
        assert!(
            decode_access_token(&wrong_config, &token).is_err(),
            "Wrong issuer MUST be rejected"
        );
    }

    #[test]
    fn rfc7519_jwt_audience_validated() {
        let config = test_config();
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        )
        .unwrap();

        let wrong_config = config.clone().with_audience("wrong-audience");
        assert!(
            decode_access_token(&wrong_config, &token).is_err(),
            "Wrong audience MUST be rejected"
        );
    }

    #[test]
    fn rfc7519_jwt_signature_validated() {
        let config = test_config();
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        )
        .unwrap();

        let wrong_config = test_config();
        assert!(
            decode_access_token(&wrong_config, &token).is_err(),
            "Wrong signature MUST be rejected"
        );
    }

    #[test]
    fn invite_token_round_trips_with_expected_purpose_and_role() {
        let config = test_config();
        let tenant_id = Uuid::new_v4();

        let token = encode_invite_token(
            &config,
            tenant_id,
            "NewUser@Example.COM",
            UserRole::Manager,
            None,
            1_800,
        )
        .unwrap();

        let claims = decode_invite_token(&config, &token).unwrap();
        assert_eq!(claims.sub, "newuser@example.com");
        assert_eq!(claims.tenant_id, tenant_id);
        assert_eq!(claims.role, UserRole::Manager);
        assert_eq!(claims.purpose, INVITE_PURPOSE);
        assert_eq!(claims.exp - claims.iat, 1_800);
    }

    #[test]
    fn invite_decoder_rejects_other_special_token_purposes() {
        let config = test_config();
        let token =
            encode_password_reset_token(&config, Uuid::new_v4(), "user@example.com", 900).unwrap();

        assert!(decode_invite_token(&config, &token).is_err());
    }

    #[test]
    fn access_jwt_rejects_signed_invalid_principal_facts() {
        let config = test_config();
        let token = encode_access_token(
            &config, Uuid::new_v4(), Uuid::new_v4(), UserRole::Customer, Uuid::new_v4(),
        ).unwrap();
        let signed_invalid = |claims: &Claims| {
            let token = encode_for_purpose(&config, JwtPurpose::Access, claims).unwrap();
            assert!(decode_access_token(&config, &token).is_err());
        };
        let mut claims = decode_access_token(&config, &token).unwrap();
        claims.jti = Some(Uuid::new_v4());
        signed_invalid(&claims);
        claims.jti = None;
        claims.scopes.push("catalog:read".to_string());
        signed_invalid(&claims);
        claims.scopes.clear();
        claims.session_id = Uuid::nil();
        signed_invalid(&claims);

        claims.grant_type = "authorization_code".to_string();
        claims.client_id = Some(Uuid::new_v4());
        signed_invalid(&claims); // Missing durable registration.
        claims.jti = Some(Uuid::nil());
        signed_invalid(&claims);
        claims.jti = Some(Uuid::new_v4());
        claims.client_id = Some(Uuid::nil());
        signed_invalid(&claims);
        claims.client_id = Some(Uuid::new_v4());
        claims.grant_type = "refresh_token".to_string();
        signed_invalid(&claims);
    }

    #[test]
    fn oauth_encoder_rejects_unsupported_grants() {
        let config = test_config();
        for grant_type in ["", "direct", "refresh_token", "unknown"] {
            assert!(encode_oauth_access_token(&config, OauthAccessTokenInput {
                app_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                role: UserRole::Customer,
                client_id: Uuid::new_v4(),
                scopes: &[],
                grant_type,
                expires_in_secs: 900,
                token_id: Uuid::new_v4(),
            }).is_err());
        }
    }

    #[test]
    fn invite_issuance_requires_a_unique_non_nil_nonce() {
        let config = test_config();
        let tenant = Uuid::new_v4();
        let issue = || encode_invite_token(
            &config, tenant, "a@example.com", UserRole::Customer, None, 900,
        ).unwrap();
        let first = issue();
        let second = issue();
        assert_ne!(first, second);
        let mut claims = decode_invite_token(&config, &first).unwrap();
        let second_claims = decode_invite_token(&config, &second).unwrap();
        assert!(!claims.jti.is_nil());
        assert!(!second_claims.jti.is_nil());
        assert_ne!(claims.jti, second_claims.jti);
        let mut payload = serde_json::to_value(&claims).unwrap();
        payload.as_object_mut().unwrap().remove("jti");
        let missing = encode_for_purpose(&config, JwtPurpose::Invite, &payload).unwrap();
        assert!(decode_invite_token(&config, &missing).is_err());
        payload["jti"] = serde_json::Value::Null;
        let null = encode_for_purpose(&config, JwtPurpose::Invite, &payload).unwrap();
        assert!(decode_invite_token(&config, &null).is_err());
        claims.jti = Uuid::nil();
        let invalid = encode_for_purpose(&config, JwtPurpose::Invite, &claims).unwrap();
        assert!(decode_invite_token(&config, &invalid).is_err());
    }

    #[test]
    fn access_claims_require_explicit_grant_type_and_scopes() {
        let config = test_config();
        let direct = encode_access_token(
            &config, Uuid::new_v4(), Uuid::new_v4(),
            UserRole::Customer, Uuid::new_v4(),
        ).unwrap();
        let oauth = encode_oauth_access_token(&config, OauthAccessTokenInput {
            app_id: Uuid::new_v4(),
            tenant_id: Uuid::new_v4(),
            role: UserRole::Customer,
            client_id: Uuid::new_v4(),
            scopes: &[],
            grant_type: "client_credentials",
            expires_in_secs: 900,
            token_id: Uuid::new_v4(),
        }).unwrap();
        for token in [direct, oauth] {
            let claims = decode_access_token(&config, &token).unwrap();
            for field in ["grant_type", "scopes"] {
                let mut payload = serde_json::to_value(&claims).unwrap();
                payload.as_object_mut().unwrap().remove(field);
                let missing = encode_for_purpose(&config, JwtPurpose::Access, &payload).unwrap();
                assert!(decode_access_token(&config, &missing).is_err());
                payload[field] = serde_json::Value::Null;
                let null = encode_for_purpose(&config, JwtPurpose::Access, &payload).unwrap();
                assert!(decode_access_token(&config, &null).is_err());
            }
        }
    }

    #[test]
    fn invite_encoder_rejects_roles_that_decoder_cannot_accept() {
        let config = test_config();
        let tenant = Uuid::new_v4();
        for (role, slug) in [
            (UserRole::Customer, " ".to_string()),
            (UserRole::Customer, "a".repeat(65)),
            (UserRole::Admin, "custom".to_string()),
        ] {
            assert!(encode_invite_token(
                &config, tenant, "a@example.com", role, Some(slug), 900,
            ).is_err());
        }
        let token = encode_invite_token(
            &config, tenant, "a@example.com", UserRole::Customer, Some("custom".to_string()), 900,
        ).unwrap();
        assert_eq!(decode_invite_token(&config, &token).unwrap().custom_role_slug.as_deref(), Some("custom"));
    }

    #[test]
    fn oauth2_direct_login_no_client_id() {
        let config = test_config();
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Admin,
            Uuid::new_v4(),
        )
        .unwrap();

        let claims = decode_access_token(&config, &token).unwrap();
        assert!(claims.client_id.is_none());
        assert!(claims.scopes.is_empty());
        assert_eq!(claims.grant_type, "direct");
    }

    #[test]
    fn oauth2_client_credentials_token_claims() {
        let config = test_config();
        let client_id = Uuid::new_v4();
        let scopes = vec!["catalog:read".to_string(), "orders:write".to_string()];

        let token = encode_oauth_access_token(
            &config,
            OauthAccessTokenInput {
                app_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                role: UserRole::Customer,
                client_id,
                scopes: &scopes,
                grant_type: "client_credentials",
                expires_in_secs: 3600,
                token_id: Uuid::new_v4(),
            },
        )
        .unwrap();

        let claims = decode_access_token(&config, &token).unwrap();
        assert_eq!(claims.client_id, Some(client_id));
        assert_eq!(claims.scopes, scopes);
        assert_eq!(claims.grant_type, "client_credentials");
    }

    #[test]
    fn oauth2_token_ttl_matches_requested() {
        let config = test_config();
        let token = encode_oauth_access_token(
            &config,
            OauthAccessTokenInput {
                app_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                role: UserRole::Customer,
                client_id: Uuid::new_v4(),
                scopes: &["catalog:read".to_string()],
                grant_type: "client_credentials",
                expires_in_secs: 3600,
                token_id: Uuid::new_v4(),
            },
        )
        .unwrap();

        let claims = decode_access_token(&config, &token).unwrap();
        assert_eq!(claims.exp - claims.iat, 3600);
    }

    #[test]
    fn encode_access_token_rejects_nil_uuids() {
        let config = test_config();
        assert!(
            encode_access_token(
                &config,
                Uuid::nil(),
                Uuid::new_v4(),
                UserRole::Admin,
                Uuid::new_v4()
            )
            .is_err()
        );
        assert!(
            encode_access_token(
                &config,
                Uuid::new_v4(),
                Uuid::nil(),
                UserRole::Admin,
                Uuid::new_v4()
            )
            .is_err()
        );
        assert!(
            encode_access_token(
                &config,
                Uuid::new_v4(),
                Uuid::new_v4(),
                UserRole::Admin,
                Uuid::nil()
            )
            .is_err()
        );
    }

    #[test]
    fn decode_access_token_trims_whitespace_and_rejects_empty() {
        let config = test_config();
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Admin,
            Uuid::new_v4(),
        )
        .unwrap();

        let padded_token = format!("   {}   \n", token);
        let claims =
            decode_access_token(&config, &padded_token).expect("padded token should decode");
        assert_eq!(claims.role, UserRole::Admin);

        assert!(decode_access_token(&config, "").is_err());
        assert!(decode_access_token(&config, "   ").is_err());
    }

    #[test]
    fn special_purpose_tokens_reject_empty_email_and_nil_tenant() {
        let config = test_config();
        let valid_tenant = Uuid::new_v4();

        assert!(encode_password_reset_token(&config, valid_tenant, "", 900).is_err());
        assert!(encode_password_reset_token(&config, valid_tenant, "   ", 900).is_err());
        assert!(
            encode_password_reset_token(&config, Uuid::nil(), "user@example.com", 900).is_err()
        );

        assert!(encode_email_verification_token(&config, valid_tenant, "", 900).is_err());
        assert!(encode_email_verification_token(&config, valid_tenant, "   ", 900).is_err());
        assert!(
            encode_email_verification_token(&config, Uuid::nil(), "user@example.com", 900).is_err()
        );

        assert!(
            encode_invite_token(&config, valid_tenant, "", UserRole::Manager, None, 900).is_err()
        );
        assert!(
            encode_invite_token(&config, valid_tenant, "   ", UserRole::Manager, None, 900)
                .is_err()
        );
        assert!(
            encode_invite_token(
                &config,
                Uuid::nil(),
                "user@example.com",
                UserRole::Manager,
                None,
                900
            )
            .is_err()
        );
    }

    #[test]
    fn special_purpose_tokens_trim_whitespace_and_reject_empty() {
        let config = test_config();
        let tenant_id = Uuid::new_v4();

        let reset_token =
            encode_password_reset_token(&config, tenant_id, "user@example.com", 900).unwrap();
        let padded_reset = format!("  {}  ", reset_token);
        assert!(decode_password_reset_token(&config, &padded_reset).is_ok());
        assert!(decode_password_reset_token(&config, "").is_err());

        let verify_token =
            encode_email_verification_token(&config, tenant_id, "user@example.com", 900).unwrap();
        let padded_verify = format!("  {}  ", verify_token);
        assert!(decode_email_verification_token(&config, &padded_verify).is_ok());
        assert!(decode_email_verification_token(&config, "").is_err());

        let invite_token = encode_invite_token(
            &config,
            tenant_id,
            "user@example.com",
            UserRole::Manager,
            None,
            900,
        )
        .unwrap();
        let padded_invite = format!("  {}  ", invite_token);
        assert!(decode_invite_token(&config, &padded_invite).is_ok());
        assert!(decode_invite_token(&config, "").is_err());
    }
}
