use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, Header, Validation, decode, encode};
use rustok_api::AuthPrincipalKind;
use rustok_core::UserRole;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[cfg(test)]
use jsonwebtoken::{EncodingKey, decode_header};
use uuid::Uuid;

use crate::config::{AuthConfig, JwtAlgorithm};
use crate::error::{AuthError, Result};
use crate::jwt_keyring::{JwtPurpose, ValidatedJwtKeyring};
#[cfg(test)]
use crate::jwt_keyring::PurposeKeyringRef;

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

    // OAuth2 extension fields (backward-compatible)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<Uuid>,
    #[serde(default)]
    pub scopes: Vec<String>,
    #[serde(default = "default_grant_type")]
    pub grant_type: String,
    /// Registered OAuth access-token id (`jti`). Direct first-party
    /// session tokens leave this absent; delegated/service OAuth tokens
    /// must carry the id of their durable `oauth_tokens` access row so
    /// revocation can fence an already-issued JWT before its TTL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<Uuid>,
}

fn default_grant_type() -> String {
    DIRECT_GRANT_TYPE.to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteClaims {
    pub sub: String,
    pub tenant_id: Uuid,
    pub role: UserRole,
    /// Tenant-defined custom role layered over the baseline `role`
    /// membership at acceptance time. Absent on tokens issued before
    /// custom-role invites existed, which therefore keep decoding.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub custom_role_slug: Option<String>,
    /// Unique issuance nonce. Previously issued invitations omit this field;
    /// the persisted exact token digest remains the single-use authority.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<Uuid>,
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
    let now = Utc::now();
    let exp = token_expiration(now, config.access_expiration)?;

    let claims = Claims {
        sub: user_id,
        tenant_id,
        role,
        session_id,
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
        client_id: None,
        scopes: Vec::new(),
        grant_type: DIRECT_GRANT_TYPE.to_string(),
        jti: None,
    };

    encode_for_purpose(config, JwtPurpose::Access, &claims)
}

pub fn encode_oauth_access_token(
    config: &AuthConfig,
    input: OauthAccessTokenInput<'_>,
) -> Result<String> {
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
    let now = Utc::now();
    let exp = token_expiration(now, input.expires_in_secs)?;

    let claims = Claims {
        sub: input.app_id,
        tenant_id: input.tenant_id,
        role: input.role,
        session_id: Uuid::nil(),
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
        client_id: Some(input.client_id),
        scopes: input.scopes.to_vec(),
        grant_type: input.grant_type.to_string(),
        jti: Some(input.token_id),
    };

    if !valid_access_subject(&claims) {
        return Err(AuthError::TokenEncodingFailed);
    }
    encode_for_purpose(config, JwtPurpose::Access, &claims)
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
        jti: Some(Uuid::new_v4()),
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
        || claims.jti.is_some_and(|id| id.is_nil())
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

fn jwt_algorithm(algorithm: &JwtAlgorithm) -> Algorithm {
    match algorithm {
        JwtAlgorithm::HS256 => Algorithm::HS256,
        JwtAlgorithm::RS256 => Algorithm::RS256,
    }
}

fn prepared_header(ring: &ValidatedJwtKeyring, purpose: JwtPurpose) -> Header {
    let mut header = Header::new(jwt_algorithm(ring.algorithm()));
    header.kid = Some(ring.active_kid().to_string());
    header.typ = Some(purpose.typ().to_string());
    header
}

fn encode_for_purpose<T: Serialize>(
    config: &AuthConfig, purpose: JwtPurpose, claims: &T,
) -> Result<String> {
    let ring = config.prepared_jwt()?.for_purpose(purpose);
    encode(&prepared_header(ring, purpose), claims, ring.signing_key())
        .map_err(|_| AuthError::TokenEncodingFailed)
}

#[cfg(test)]
fn jwt_header(config: &AuthConfig, purpose: JwtPurpose) -> Header {
    prepared_header(config.prepared_jwt().unwrap().for_purpose(purpose), purpose)
}

#[cfg(test)]
fn encoding_key(config: &AuthConfig, purpose: JwtPurpose) -> Result<&EncodingKey> {
    Ok(config.prepared_jwt()?.for_purpose(purpose).signing_key())
}

/// Only the JOSE fields emitted by Auth are supported. In particular, null
/// kid is not equivalent to an omitted kid during the access migration.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthJwtHeader {
    alg: Algorithm,
    typ: String,
    #[serde(default, deserialize_with = "deserialize_present_kid")]
    kid: Option<String>,
}

fn deserialize_present_kid<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
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
    let selected = config.prepared_jwt()?.for_purpose(purpose);
    if header.alg != jwt_algorithm(selected.algorithm())
        || header.typ != purpose.typ()
        || (purpose != JwtPurpose::Access && header.kid.is_none())
    {
        return Err(AuthError::InvalidAccessToken);
    }
    let (key, cutoff) = selected.select(header.kid.as_deref(), Utc::now().timestamp())
        .ok_or(AuthError::InvalidAccessToken)?;
    let validation = strict_jwt_validation(config, selected.algorithm());
    let claims = decode::<T>(token, key, &validation)
        .map_err(|_| AuthError::InvalidAccessToken)?.claims;
    if cutoff.is_some_and(|until| Utc::now().timestamp() >= until) {
        return Err(AuthError::InvalidAccessToken);
    }
    Ok(claims)
}

// ─── Validation ──────────────────────────────────────────────────────

fn valid_token_times(iat: usize, exp: usize) -> bool {
    let Ok(now) = usize::try_from(Utc::now().timestamp()) else {
        return false;
    };
    iat > 0 && iat <= now && iat < exp && now < exp
}

fn strict_jwt_validation(config: &AuthConfig, algorithm: &JwtAlgorithm) -> Validation {
    let mut validation = Validation::new(jwt_algorithm(algorithm));
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
pub(crate) const TEST_RSA_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvgIBADANBgkqhkiG9w0BAQEFAASCBKgwggSkAgEAAoIBAQDdua49SRdQT5tH
privGhGOztfH1Tor39Zq1fm+oM2v9DksP3GLsllHP8UklUPWQbZTMtsJyoPlPqjH
UDcBMTyGkiAHvIDEgRz741z6uOrJkZNH2wyV7EFjWhdaDcNausVTof5nOyOXOZLQ
6Z5WBg4YxiBFRFPMrk284gUdy+ibmZ7Pj6VGPbB4Z0pTD+mkQZUXmebQsoIS7MUi
rP1DZa0aB75Ys/fD9tJ8Xu51Jyn3EtcynDLbXHczBNTvFakXo1mMQnGozkelNwKI
mJT08HoN4CRzWzUhd/72xwmSLUbalpmZY1ZBV6Bl2gbtyQeQOOfsqePHpWbUcNSy
1DGq7GjHAgMBAAECggEAJ5jZfShoeXc+C/XCVcMaD57w+kciNGOtLzc3esvM7/d1
nmlWJdScDRVeZ8Igc0sY/JLAe2cnVvFxwuaYbCYW4RGHltobRPyp6HIaUMxlYcoV
u2drP/sJUmzsbrC2iqWASAdOH7F4EbG+foC6PjKmodYAPV6OeKdISssyjrezutXH
ZGSQyXM/5+T+fiZEaJr1X0MEXKDU/GhmVLJ7PqPSRjZJFTYuD53uYqG4QjJBdOSI
ttShhImjkbIbDjVlF6Ok3CgNScOfsObSGnv4ajvjVKpNtBLw+eN4JF3l0o8yI0dl
sRrEaZ3qxYvQNRtjFzzB5yM3cWaQesZpVMJTubjOAQKBgQDxRvI+9RuwJSw3brkt
Im0qxd0jFU9uzYLEEE28eo/+2KLxMo+7JllgEXj5ZDCMt+sjn1kOw55RtQ//CDcx
V0XQ140icmQPiMAray+WVGv6BXfnQFF2xzqo24UQsuCJ+mtQAeQZYWJjaPSKvOd9
ysY/cNdmje0qmwfnAc9usbJhYQKBgQDrQU9rQoVcpbcsBU50ye/Bwx4mmubNVp6d
OFNbAeZIuIt87inDwCygbP7qqBf0wbx9Qcc+1u4W5T9GO+aeqZn6zJLOYqpn9SAo
dBGdYDRpReDAH0A6T0sdlzBAQVr70LytUjGZhRdDj7zwLzpAC8tZey9vpppFpN+m
7dUn2aFzJwKBgQDebIjlgRAFUj9w2qHa+eGpjL5PmVWgz9O860q+dj5IsW2E7ReT
b8b0ySa8waAAGYyrSjrPYYaRzFjywqAe3FWAMTXqi4myyF5fqHA2JZ1k36WpiaGP
3ho1kCkbO8vDZxeGqjedLimFezv0qjC9xjD8SwpHgI8it8iRLRoM8cOAAQKBgHlV
eOmgKHpNOfjpT7qqgA7WXJGaqNlVCH+cElnI1AXDsKWhjEbasemX7a4HPjvNRDLy
HxpI7gk++XB26o4AeVtB8aGif7MYWRqkKoWZnc6B7NYKCC1KwjojxQ4O5ycjVHyr
/MrqOsJsuwzBvvBTZPDkuOWD7uNmkrdcyOhBtaRXAoGBAO/Cxvk7kLKxBctjSijt
burjOIQ5oN/HmeSHm+QHXffXEzBWfQR5Yc4VXIuxsXb4+RdiUqswLzF+hXMa8/O5
BR1yuIKlL4tKQxmoOx3+TfCbRAOwSfdvsxzIfFBrm1aavh/7Y5TNOzDnYlfD38S6
t18YRhvA80STyqQJWI3Tg7sg
-----END PRIVATE KEY-----"#;

#[cfg(test)]
pub(crate) const TEST_RSA_PUBLIC_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA3bmuPUkXUE+bR6a4rxoR
js7Xx9U6K9/WatX5vqDNr/Q5LD9xi7JZRz/FJJVD1kG2UzLbCcqD5T6ox1A3ATE8
hpIgB7yAxIEc++Nc+rjqyZGTR9sMlexBY1oXWg3DWrrFU6H+ZzsjlzmS0OmeVgYO
GMYgRURTzK5NvOIFHcvom5mez4+lRj2weGdKUw/ppEGVF5nm0LKCEuzFIqz9Q2Wt
Gge+WLP3w/bSfF7udScp9xLXMpwy21x3MwTU7xWpF6NZjEJxqM5HpTcCiJiU9PB6
DeAkc1s1IXf+9scJki1G2paZmWNWQVegZdoG7ckHkDjn7Knjx6Vm1HDUstQxquxo
xwIDAQAB
-----END PUBLIC KEY-----"#;

// Independently generated, public test-only RSA fixture for rotation regression.
#[cfg(test)]
const TEST_RSA_NEXT_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
MIIEvAIBADANBgkqhkiG9w0BAQEFAASCBKYwggSiAgEAAoIBAQCtuECvu/mgwfjn
a3Z4yAlDz5wl7HnvjbJ6ja6jbUpRtQXemkOVkyclEZPjh8Dr8f7KHKLS63MHEQ+P
7hb4c/KbOfWL3YYEDVj/ghoaFiliCqaYwuJkRBqyUEI9YCr3tQ4lVOHNWv+YZets
r8Oe2vnnSbIW2pfkFHMys/iS8kHW0LV1F5HLrbDhM8mK6cpHsvlpWYyA2vqE75jz
VF8tDlJIJEMj4RthCDSoJAjr8D6w7DPWEs8o/SeHO+t38NY5ASxH7t8gNdC/H7bC
h6W/SYHW6c3Dcg7P9cjbLmEGFGeAaK+AeNWssoqclkpgrhDepKDUtdjqBUc5bdWc
tYLs3ZAtAgMBAAECggEACxEU1d0kvu/8ekmnR3R+2poMOYJCFlexigsil/irZl8g
25h6O7ofIUZoZHNOf/lvoNKIMOWc81NL6YBiaDuCtg7QLLPVCMcM52ITZZwJ0Nwa
1tp7b6XDCZ8BSUWqFxOqrgglRl7Ir522jLr5vFH435obRUdxq67907oRWNMkDJou
BAwUG9Upb1eCsygWTUBIMIRgnjJDBvEJTl7BjjqSXWz6XDdE3ANXU1M45DOd+/pZ
bZbBPepIOLpPW6D9jD3Ph6+8YbsjyAWfXj9TY8b/zi76EdNki/vdBuFlw7x4Ptyx
hC6bqXmQl8vGfoBZ0/ZJZxnAs8H552cWNJVA6aOQQQKBgQDyWd4IyrOP+PoE2ulS
ylH6+I47ior4c400N4Vcc269IB5Hb1io5IddG690MUq7qu3lG7pqcLm8SQIzV8OS
wmUQ24HpQklZDaw4wZX+ADvoqZZPhpiZF3s9ocr0od9I4reDnndf38LITiTxpBfv
/2ZcRIp/38MKZybc95pRWtmf3QKBgQC3gOIWSNMNj6/NOyTcdW7izH1/CaKHDYR4
g6Nbc8Zr49FDb0RhXItt26TTEBd9SM8RecRSh1x9RQ93YNhjs7iiOt1pi0wXWoon
3Fs0ZuqM5wIrt3SxTEop7eFzzXmcubfFPsVwpRD9JiJ5hkTNJ5c3ikX4vZIIP8pl
/nqStCPUkQKBgCJ8o1oA7lh4h6uVM0qyu8PJBwR2DJyaZOefVvz2+DU1ktl7gNRX
4ONhAzC3qvjVtBC8R2TKMcQXZ5bC0HPb8eHaOrxk0Gc5yN4yvhNtGRlcuKUMdD6k
1sbIdUcmmc2XZaH3NOmRv/gdFqjEPxvJ7lkG7+aOgwTB9QfQvEcDdKCJAoGAJE1N
7MSpZARp7frmVuqwbhOwPAKG8Zq9+y1nkr77oPpnuqR3bZ9mQexy1gJX80QFGtrw
+LaomGuiHKa/CDJUs1tb0/ZnrjMOI8aXEG3PtuI4breTUUPbZCIYONWMWnHvJgk9
/APbsYjqxwnF8KQFaUpkf0nZauJbwwOt8fnLINECgYA2rJyIm4apFoF3Z9DgwEV4
ImpSkI1ch7GUvPaV/+MLbAQMQSd369vs5vqhOJq1uzNAEaydN/hCnoKJ2MWiMBSk
i/KgNm0W0/oarQJw+IkPQLGYsbQqt4Aajz7x3PvV+/5OcUKcVaJvFB4MHMviDlUN
D2G10GqpzBPKSbwsZInOTw==
-----END PRIVATE KEY-----"#;

#[cfg(test)]
const TEST_RSA_NEXT_PUBLIC_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEArbhAr7v5oMH452t2eMgJ
Q8+cJex5742yeo2uo21KUbUF3ppDlZMnJRGT44fA6/H+yhyi0utzBxEPj+4W+HPy
mzn1i92GBA1Y/4IaGhYpYgqmmMLiZEQaslBCPWAq97UOJVThzVr/mGXrbK/Dntr5
50myFtqX5BRzMrP4kvJB1tC1dReRy62w4TPJiunKR7L5aVmMgNr6hO+Y81RfLQ5S
SCRDI+EbYQg0qCQI6/A+sOwz1hLPKP0nhzvrd/DWOQEsR+7fIDXQvx+2woelv0mB
1unNw3IOz/XI2y5hBhRngGivgHjVrLKKnJZKYK4Q3qSg1LXY6gVHOW3VnLWC7N2Q
LQIDAQAB
-----END PUBLIC KEY-----"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> AuthConfig {
        AuthConfig::new("test-secret-key-for-unit-tests-only-32bytes!".to_string())
    }


    #[test]
    fn jwt_keyring_rotation_selects_one_key_and_enforces_retirement() {
        use serde_json::json;
        let old = test_config();
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
        let settings = |until| json!({
            "active_kid": "next",
            "active_hs256_secret": "new-jwt-signing-secret-only-for-tests-123456",
            "verification_keys": [{
                "kid": "default", "material": PurposeKeyringRef::for_config(&old, JwtPurpose::Access).active_material().unwrap().as_ref(), "verify_until": until
            }],
            "legacy_without_kid": {"kid": "default", "verify_until": until}
        });
        let rotated = crate::config::build_auth_config_with_env(
            old.secret().to_string(), 900,
            crate::config::AuthSettingsOverrides {
                jwt_keyring_env: Some("JWT_KEYS".to_string()),
                ..Default::default()
            },
            |_| Ok(settings(now + 3600).to_string()),
        ).unwrap();
        assert_eq!(decode_access_token(&rotated, &token).unwrap().sub, user);
        assert_eq!(decode_access_token(&rotated, &oauth).unwrap().grant_type, "client_credentials");
        assert!(decode_access_token(&rotated, &legacy).is_ok());
        let fresh = encode_access_token(&rotated, user, tenant, UserRole::Customer, session).unwrap();
        assert_eq!(decode_header(&fresh).unwrap().kid.as_deref(), Some("next"));
        assert!(decode_access_token(&rotated, &fresh).is_ok());
        assert!(decode_access_token(&old, &fresh).is_err());
        for kid in ["unknown", "", "default"] {
            let mut header = jwt_header(&rotated, JwtPurpose::Access);
            header.kid = Some(kid.to_string());
            let forged = encode(&header, &old_claims, encoding_key(&rotated, JwtPurpose::Access).unwrap()).unwrap();
            assert!(decode_access_token(&rotated, &forged).is_err());
        }
        let mut retired = rotated.clone();
        retired.set_test_jwt_keyring(crate::jwt_keyring::JwtKeyring::from_json(
            &settings(now - 1).to_string()
        ).unwrap());
        assert!(decode_access_token(&retired, &token).is_err());
        assert!(decode_access_token(&retired, &legacy).is_err());
        assert!(decode_access_token(&retired, &fresh).is_ok());
    }

    #[test]
    fn purpose_rotation_is_independent_and_rejects_cross_signed_claims() {
        use serde_json::json;
        let old = test_config();
        let tenant = Uuid::new_v4();
        let reset = encode_password_reset_token(&old, tenant, "a@example.com", 900).unwrap();
        let verify = encode_email_verification_token(&old, tenant, "a@example.com", 900).unwrap();
        let invite = encode_invite_token(&old, tenant, "a@example.com", UserRole::Customer, None, 900).unwrap();
        let access = encode_access_token(&old, Uuid::new_v4(), tenant, UserRole::Customer, Uuid::new_v4()).unwrap();
        let reset_claims = decode_password_reset_token(&old, &reset).unwrap();
        let access_claims = decode_access_token(&old, &access).unwrap();
        let mut rotated = old.clone();
        let old_reset_key = PurposeKeyringRef::for_config(&old, JwtPurpose::PasswordReset)
            .active_material().unwrap();
        let settings = |until| json!({
            "password_reset": {"keyring": {
                "active_kid": "reset-next",
                "active_hs256_secret": "independent-reset-signing-key-for-tests-12345",
                "verification_keys": [{
                    "kid": "reset-default", "material": old_reset_key.as_ref(),
                    "verify_until": until
                }]
            }}
        });
        let now = Utc::now().timestamp();
        rotated.set_test_special_jwt_keyrings(crate::jwt_keyring::SpecialJwtKeyrings::from_json(
            &settings(now + 3600).to_string()
        ).unwrap());
        crate::config::validate_auth_config(&rotated).unwrap();
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
        rotated.set_test_special_jwt_keyrings(crate::jwt_keyring::SpecialJwtKeyrings::from_json(
            &settings(now - 1).to_string()
        ).unwrap());
        assert!(decode_password_reset_token(&rotated, &reset).is_err());
        assert!(decode_password_reset_token(&rotated, &fresh).is_ok());
        assert!(decode_email_verification_token(&rotated, &verify).is_ok());
        assert!(decode_invite_token(&rotated, &invite).is_ok());
        assert!(decode_access_token(&rotated, &access).is_ok());
    }

    #[test]
    fn previous_shared_access_key_can_only_migrate_access_tokens() {
        use serde_json::json;
        let old = test_config();
        let tenant = Uuid::new_v4();
        let access = encode_access_token(&old, Uuid::new_v4(), tenant, UserRole::Customer, Uuid::new_v4()).unwrap();
        let claims = decode_access_token(&old, &access).unwrap();
        let mut old_header = Header::new(Algorithm::HS256);
        old_header.kid = Some("default".to_string());
        let old_key = EncodingKey::from_secret(old.secret().as_bytes());
        let legacy_access = encode(&old_header, &claims, &old_key).unwrap();
        let mut migrated = old.clone();
        migrated.set_test_jwt_keyring(crate::jwt_keyring::JwtKeyring::from_json(&json!({
            "active_kid": "access-next",
            "verification_keys": [{
                "kid": "default", "material": old.secret(),
                "verify_until": Utc::now().timestamp() + 3600
            }]
        }).to_string()).unwrap());
        crate::config::validate_auth_config(&migrated).unwrap();
        assert!(decode_access_token(&migrated, &legacy_access).is_ok());
        let reset = encode_password_reset_token(&old, tenant, "a@example.com", 900).unwrap();
        let reset_claims = decode_password_reset_token(&old, &reset).unwrap();
        let legacy_reset = encode(&old_header, &reset_claims, &old_key).unwrap();
        assert!(decode_password_reset_token(&migrated, &legacy_reset).is_err());
    }

    #[test]
    fn direct_configuration_cannot_bypass_cross_ring_key_isolation() {
        let mut config = test_config();
        let shared = "independent-but-reused-signing-material-test-123456";
        let tenant = Uuid::new_v4();
        config.set_test_jwt_keyring(crate::jwt_keyring::JwtKeyring::from_json(
            &serde_json::json!({
                "active_kid": "access-next",
                "active_hs256_secret": shared
            }).to_string()
        ).unwrap());
        let token = encode_access_token(&config, Uuid::new_v4(), tenant, UserRole::Customer, Uuid::new_v4()).unwrap();
        config.set_test_special_jwt_keyrings(crate::jwt_keyring::SpecialJwtKeyrings::from_json(
            &serde_json::json!({
                "invite": {"keyring": {
                    "active_kid": "invite-next",
                    "active_hs256_secret": shared
                }}
            }).to_string()
        ).unwrap());
        assert!(decode_access_token(&config, &token).is_err());
        assert!(encode_invite_token(&config, tenant, "a@example.com", UserRole::Customer, None, 900).is_err());
    }

    #[test]
    fn rs256_rotation_stages_promotes_and_retires_distinct_keys() {
        use serde_json::json;
        use crate::jwt_keyring::JwtKeyring;

        let now = Utc::now().timestamp();
        let user = Uuid::new_v4();
        let tenant = Uuid::new_v4();
        let session = Uuid::new_v4();
        let old = rs256_test_config();
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

        let mut staged = old.clone();
        staged.set_test_jwt_keyring(JwtKeyring::from_json(&json!({
            "active_kid": "default",
            "verification_keys": [{
                "kid": "rsa-next", "material": TEST_RSA_NEXT_PUBLIC_KEY,
                "verify_until": now + 3600
            }]
        }).to_string()).unwrap());
        crate::config::validate_auth_config(&staged).unwrap();
        assert_eq!(decode_header(&issue(&staged)).unwrap().kid.as_deref(), Some("default"));

        let mut promoted = old.clone().with_rs256(TEST_RSA_NEXT_PRIVATE_KEY, TEST_RSA_NEXT_PUBLIC_KEY);
        let history = |until| json!({
            "active_kid": "rsa-next",
            "verification_keys": [{
                "kid": "default", "material": TEST_RSA_PUBLIC_KEY,
                "verify_until": until
            }],
            "legacy_without_kid": {"kid": "default", "verify_until": until}
        });
        promoted.set_test_jwt_keyring(JwtKeyring::from_json(&history(now + 3600).to_string()).unwrap());
        crate::config::validate_auth_config(&promoted).unwrap();
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
        assert!(decode_access_token(&promoted, &legacy).is_ok());
        for wrong_id in ["rsa-next", "unknown"] {
            let mut wrong_header = jwt_header(&old, JwtPurpose::Access);
            wrong_header.kid = Some(wrong_id.to_string());
            let wrong = encode(&wrong_header, &claims, encoding_key(&old, JwtPurpose::Access).unwrap()).unwrap();
            assert!(decode_access_token(&promoted, &wrong).is_err());
        }
        assert!(decode_password_reset_token(&promoted, &reset).is_ok());

        let mut retired = promoted.clone();
        retired.set_test_jwt_keyring(JwtKeyring::from_json(&history(now - 1).to_string()).unwrap());
        assert!(decode_access_token(&retired, &old_token).is_err());
        assert!(decode_access_token(&retired, &old_oauth).is_err());
        assert!(decode_access_token(&retired, &legacy).is_err());
        assert!(decode_access_token(&retired, &fresh).is_ok());
        assert!(decode_password_reset_token(&retired, &reset).is_ok());
        // Immutable snapshots continue to behave according to their own cutoffs.
        assert!(decode_access_token(&promoted, &old_token).is_ok());
    }

    #[test]
    fn independent_rsa_special_ring_does_not_change_hs256_access() {
        let mut config = test_config();
        config.set_test_special_jwt_keyrings(crate::jwt_keyring::SpecialJwtKeyrings::from_json(
            &serde_json::json!({
                "invite": {
                    "algorithm": "RS256",
                    "keyring": {"active_kid": "invite-rsa"},
                    "rsa_private_key_pem": TEST_RSA_PRIVATE_KEY,
                    "rsa_public_key_pem": TEST_RSA_PUBLIC_KEY
                }
            }).to_string()
        ).unwrap());
        crate::config::validate_auth_config(&config).unwrap();
        let tenant = Uuid::new_v4();
        let invite = encode_invite_token(&config, tenant, "a@example.com", UserRole::Customer, None, 900).unwrap();
        assert_eq!(decode_header(&invite).unwrap().alg, Algorithm::RS256);
        assert!(decode_invite_token(&config, &invite).is_ok());
        let access = encode_access_token(&config, Uuid::new_v4(), tenant, UserRole::Customer, Uuid::new_v4()).unwrap();
        assert_eq!(decode_header(&access).unwrap().alg, Algorithm::HS256);
        let duplicated = config.with_rs256(TEST_RSA_PRIVATE_KEY, TEST_RSA_PUBLIC_KEY);
        assert!(crate::config::validate_auth_config(&duplicated).is_err());
    }

    fn rs256_test_config() -> AuthConfig {
        test_config().with_rs256(TEST_RSA_PRIVATE_KEY, TEST_RSA_PUBLIC_KEY)
    }

    #[test]
    fn hs256_weak_secret_fails_closed_at_key_materialization() {
        // AuthConfig can be built without build_auth_config; signing or
        // verifying with a sub-32-byte secret must still be impossible.
        let weak = AuthConfig::new("short-secret".to_string());
        let encode = encode_access_token(
            &weak,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        );
        assert!(encode.is_err());

        let strong = test_config();
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
    fn hs256_whitespace_padded_secret_fails_closed() {
        // 31 real bytes with an interior space: `trim().len()` counts 32 and
        // would pass, but whitespace must not credit toward the minimum.
        let padded = AuthConfig::new(format!("{} {}", "a".repeat(16), "b".repeat(15)));
        let encode = encode_access_token(
            &padded,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        );
        assert!(encode.is_err());
        assert!(!crate::config::hs256_secret_has_min_length(padded.secret()));
        assert!(crate::config::hs256_secret_has_min_length(
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
    fn hs256_access_token_header_matches_config() {
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
        assert_eq!(header.alg, Algorithm::HS256);
    }

    #[test]
    fn rs256_access_token_round_trips_and_header_matches_config() {
        let config = rs256_test_config();
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
    fn rs256_access_token_is_not_accepted_as_hs256() {
        let rs256_config = rs256_test_config();
        let token = encode_access_token(
            &rs256_config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        )
        .unwrap();

        assert!(decode_access_token(&test_config(), &token).is_err());
    }

    #[test]
    fn rs256_access_with_default_hs256_special_purposes_round_trips() {
        let config = rs256_test_config();
        let tenant_id = Uuid::new_v4();

        let password_token =
            encode_password_reset_token(&config, tenant_id, "USER@example.com", 900).unwrap();
        let password_claims = decode_password_reset_token(&config, &password_token).unwrap();
        assert_eq!(password_claims.sub, "user@example.com");

        let verification_token =
            encode_email_verification_token(&config, tenant_id, "USER@example.com", 900).unwrap();
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
        let invite_claims = decode_invite_token(&config, &invite_token).unwrap();
        assert_eq!(invite_claims.sub, "invited@example.com");
        assert_eq!(invite_claims.role, UserRole::Admin);
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
            r#"{"alg":"HS256","typ":"JWT","kid":null}"#,
            r#"{"alg":"HS256","typ":"JWT","kid":"a","kid":"b"}"#,
            r#"{"alg":"HS256","typ":"JWT","kid":"a","crit":["b64"],"b64":false}"#,
            r#"{"alg":"HS256","typ":"JWT","kid":"a","jku":"https://example.invalid"}"#,
            r#"{"alg":"HS256","typ":"JWT","kid":"a","unknown":1}"#,
            r#"{"alg":"HS256","kid":"a"}"#,
        ] {
            let token = format!("{}.e30.signature", URL_SAFE_NO_PAD.encode(raw));
            assert!(auth_jwt_header(&token).is_err());
        }
        let omitted = format!("{}.e30.signature", URL_SAFE_NO_PAD.encode(r#"{"alg":"HS256","typ":"JWT"}"#));
        assert!(auth_jwt_header(&omitted).unwrap().kid.is_none());
        let oversized = format!("{}.e30.signature", "a".repeat(1025));
        assert!(auth_jwt_header(&oversized).is_err());
    }

    #[test]
    fn rsa_direct_signer_rejects_mismatched_pair() {
        let wrong = TEST_RSA_PUBLIC_KEY.replacen("3bmu", "3cmu", 1);
        let config = test_config().with_rs256(TEST_RSA_PRIVATE_KEY, wrong);
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

        let wrong_config = test_config().with_issuer("wrong-issuer");
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

        let wrong_config = test_config().with_audience("wrong-audience");
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

        let wrong_config = AuthConfig::new("completely-different-secret-key-32bytes!!".to_string());
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
    fn invite_issuance_has_a_unique_nonce_and_preserves_legacy_tokens() {
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
        assert!(claims.jti.is_some_and(|id| !id.is_nil()));
        assert!(second_claims.jti.is_some_and(|id| !id.is_nil()));
        assert_ne!(claims.jti, second_claims.jti);
        claims.jti = None;
        let legacy = encode_for_purpose(&config, JwtPurpose::Invite, &claims).unwrap();
        assert!(decode_invite_token(&config, &legacy).unwrap().jti.is_none());
        claims.jti = Some(Uuid::nil());
        let invalid = encode_for_purpose(&config, JwtPurpose::Invite, &claims).unwrap();
        assert!(decode_invite_token(&config, &invalid).is_err());
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
