use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use rustok_core::UserRole;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::{AuthConfig, JwtAlgorithm};
use crate::error::{AuthError, Result};

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
}

fn default_grant_type() -> String {
    "direct".to_string()
}

#[derive(Debug, Serialize, Deserialize)]
pub struct InviteClaims {
    pub sub: String,
    pub tenant_id: Uuid,
    pub role: UserRole,
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
    let now = Utc::now();
    let exp = now + Duration::seconds(config.access_expiration as i64);

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
        grant_type: "direct".to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &encoding_key(config)?,
    )
    .map_err(|_| AuthError::TokenEncodingFailed)
}

pub fn encode_oauth_access_token(
    config: &AuthConfig,
    app_id: Uuid,
    tenant_id: Uuid,
    client_id: Uuid,
    scopes: &[String],
    grant_type: &str,
    expires_in_secs: u64,
) -> Result<String> {
    let now = Utc::now();
    let exp = now + Duration::seconds(expires_in_secs as i64);

    let claims = Claims {
        sub: app_id,
        tenant_id,
        role: UserRole::Customer,
        session_id: Uuid::nil(),
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
        client_id: Some(client_id),
        scopes: scopes.to_vec(),
        grant_type: grant_type.to_string(),
    };

    encode(
        &Header::default(),
        &claims,
        &encoding_key(config)?,
    )
    .map_err(|_| AuthError::TokenEncodingFailed)
}

pub fn decode_access_token(config: &AuthConfig, token: &str) -> Result<Claims> {
    let validation = strict_jwt_validation(config);

    decode::<Claims>(
        token,
        &decoding_key(config)?,
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| AuthError::InvalidAccessToken)
}

// ─── Special-purpose tokens ──────────────────────────────────────────

pub fn encode_password_reset_token(
    config: &AuthConfig,
    tenant_id: Uuid,
    email: &str,
    ttl_seconds: u64,
) -> Result<String> {
    let now = Utc::now();
    let exp = now + Duration::seconds(ttl_seconds as i64);

    let claims = PasswordResetClaims {
        sub: email.to_lowercase(),
        tenant_id,
        purpose: "password_reset".to_string(),
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &encoding_key(config)?,
    )
    .map_err(|_| AuthError::TokenEncodingFailed)
}

pub fn decode_password_reset_token(
    config: &AuthConfig,
    token: &str,
) -> Result<PasswordResetClaims> {
    let validation = strict_jwt_validation(config);

    let claims = decode::<PasswordResetClaims>(
        token,
        &decoding_key(config)?,
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| AuthError::InvalidResetToken)?;

    if claims.purpose != "password_reset" {
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
    let now = Utc::now();
    let exp = now + Duration::seconds(ttl_seconds as i64);

    let claims = EmailVerificationClaims {
        sub: email.to_lowercase(),
        tenant_id,
        purpose: "email_verification".to_string(),
        iss: config.issuer.clone(),
        aud: config.audience.clone(),
        exp: exp.timestamp() as usize,
        iat: now.timestamp() as usize,
    };

    encode(
        &Header::default(),
        &claims,
        &encoding_key(config)?,
    )
    .map_err(|_| AuthError::TokenEncodingFailed)
}

pub fn decode_email_verification_token(
    config: &AuthConfig,
    token: &str,
) -> Result<EmailVerificationClaims> {
    let validation = strict_jwt_validation(config);

    let claims = decode::<EmailVerificationClaims>(
        token,
        &decoding_key(config)?,
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| AuthError::InvalidVerificationToken)?;

    if claims.purpose != "email_verification" {
        return Err(AuthError::InvalidVerificationToken);
    }

    Ok(claims)
}

pub fn decode_invite_token(config: &AuthConfig, token: &str) -> Result<InviteClaims> {
    let validation = strict_jwt_validation(config);

    let claims = decode::<InviteClaims>(
        token,
        &decoding_key(config)?,
        &validation,
    )
    .map(|data| data.claims)
    .map_err(|_| AuthError::InvalidInviteToken)?;

    if claims.purpose != "invite" {
        return Err(AuthError::InvalidInviteToken);
    }

    Ok(claims)
}

// ─── Key helpers ─────────────────────────────────────────────────────

/// Build the encoding key for the configured algorithm.
fn encoding_key(config: &AuthConfig) -> Result<EncodingKey> {
    match config.algorithm {
        JwtAlgorithm::HS256 => Ok(EncodingKey::from_secret(config.secret.as_bytes())),
        JwtAlgorithm::RS256 => {
            let pem = config.rsa_private_key_pem.as_deref().ok_or_else(|| {
                AuthError::Internal("RS256 requires rsa_private_key_pem".to_string())
            })?;
            EncodingKey::from_rsa_pem(pem.as_bytes())
                .map_err(|e| AuthError::Internal(format!("Invalid RSA private key: {e}")))
        }
    }
}

/// Build the decoding key for the configured algorithm.
fn decoding_key(config: &AuthConfig) -> Result<DecodingKey> {
    match config.algorithm {
        JwtAlgorithm::HS256 => Ok(DecodingKey::from_secret(config.secret.as_bytes())),
        JwtAlgorithm::RS256 => {
            let pem = config.rsa_public_key_pem.as_deref().ok_or_else(|| {
                AuthError::Internal("RS256 requires rsa_public_key_pem".to_string())
            })?;
            DecodingKey::from_rsa_pem(pem.as_bytes())
                .map_err(|e| AuthError::Internal(format!("Invalid RSA public key: {e}")))
        }
    }
}

// ─── Validation ──────────────────────────────────────────────────────

fn strict_jwt_validation(config: &AuthConfig) -> Validation {
    let algorithm = match config.algorithm {
        JwtAlgorithm::HS256 => Algorithm::HS256,
        JwtAlgorithm::RS256 => Algorithm::RS256,
    };
    let mut validation = Validation::new(algorithm);
    validation.validate_exp = true;
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

    fn test_config() -> AuthConfig {
        AuthConfig {
            secret: "test-secret-key-for-unit-tests-only-32bytes!".to_string(),
            access_expiration: 900,
            refresh_expiration: 2_592_000,
            issuer: "rustok".to_string(),
            audience: "rustok-admin".to_string(),
            algorithm: JwtAlgorithm::HS256,
            rsa_private_key_pem: None,
            rsa_public_key_pem: None,
        }
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
    fn rfc7519_jwt_expiration_enforced() {
        let config = AuthConfig {
            access_expiration: 0,
            ..test_config()
        };
        let token = encode_access_token(
            &config,
            Uuid::new_v4(),
            Uuid::new_v4(),
            UserRole::Customer,
            Uuid::new_v4(),
        )
        .unwrap();

        let result = decode_access_token(&config, &token);
        assert!(result.is_err(), "Expired JWT MUST be rejected");
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

        let wrong_config = AuthConfig {
            issuer: "wrong-issuer".to_string(),
            ..test_config()
        };
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

        let wrong_config = AuthConfig {
            audience: "wrong-audience".to_string(),
            ..test_config()
        };
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

        let wrong_config = AuthConfig {
            secret: "completely-different-secret-key-32bytes!!".to_string(),
            ..test_config()
        };
        assert!(
            decode_access_token(&wrong_config, &token).is_err(),
            "Wrong signature MUST be rejected"
        );
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
            Uuid::new_v4(),
            Uuid::new_v4(),
            client_id,
            &scopes,
            "client_credentials",
            3600,
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
            Uuid::new_v4(),
            Uuid::new_v4(),
            Uuid::new_v4(),
            &["catalog:read".to_string()],
            "client_credentials",
            3600,
        )
        .unwrap();

        let claims = decode_access_token(&config, &token).unwrap();
        assert_eq!(claims.exp - claims.iat, 3600);
    }
}
