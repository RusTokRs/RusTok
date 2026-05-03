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

    encode(&jwt_header(config), &claims, &encoding_key(config)?)
        .map_err(|_| AuthError::TokenEncodingFailed)
}

pub fn encode_oauth_access_token(
    config: &AuthConfig,
    input: OauthAccessTokenInput<'_>,
) -> Result<String> {
    let now = Utc::now();
    let exp = now + Duration::seconds(input.expires_in_secs as i64);

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
    };

    encode(&jwt_header(config), &claims, &encoding_key(config)?)
        .map_err(|_| AuthError::TokenEncodingFailed)
}

pub struct OauthAccessTokenInput<'a> {
    pub app_id: Uuid,
    pub tenant_id: Uuid,
    pub role: UserRole,
    pub client_id: Uuid,
    pub scopes: &'a [String],
    pub grant_type: &'a str,
    pub expires_in_secs: u64,
}

pub fn decode_access_token(config: &AuthConfig, token: &str) -> Result<Claims> {
    let validation = strict_jwt_validation(config);

    decode::<Claims>(token, &decoding_key(config)?, &validation)
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

    encode(&jwt_header(config), &claims, &encoding_key(config)?)
        .map_err(|_| AuthError::TokenEncodingFailed)
}

pub fn decode_password_reset_token(
    config: &AuthConfig,
    token: &str,
) -> Result<PasswordResetClaims> {
    let validation = strict_jwt_validation(config);

    let claims = decode::<PasswordResetClaims>(token, &decoding_key(config)?, &validation)
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

    encode(&jwt_header(config), &claims, &encoding_key(config)?)
        .map_err(|_| AuthError::TokenEncodingFailed)
}

pub fn decode_email_verification_token(
    config: &AuthConfig,
    token: &str,
) -> Result<EmailVerificationClaims> {
    let validation = strict_jwt_validation(config);

    let claims = decode::<EmailVerificationClaims>(token, &decoding_key(config)?, &validation)
        .map(|data| data.claims)
        .map_err(|_| AuthError::InvalidVerificationToken)?;

    if claims.purpose != "email_verification" {
        return Err(AuthError::InvalidVerificationToken);
    }

    Ok(claims)
}

pub fn decode_invite_token(config: &AuthConfig, token: &str) -> Result<InviteClaims> {
    let validation = strict_jwt_validation(config);

    let claims = decode::<InviteClaims>(token, &decoding_key(config)?, &validation)
        .map(|data| data.claims)
        .map_err(|_| AuthError::InvalidInviteToken)?;

    if claims.purpose != "invite" {
        return Err(AuthError::InvalidInviteToken);
    }

    Ok(claims)
}

// ─── Key helpers ─────────────────────────────────────────────────────

/// Build the encoding key for the configured algorithm.
fn jwt_header(config: &AuthConfig) -> Header {
    Header::new(jwt_algorithm(config))
}

fn jwt_algorithm(config: &AuthConfig) -> Algorithm {
    match config.algorithm {
        JwtAlgorithm::HS256 => Algorithm::HS256,
        JwtAlgorithm::RS256 => Algorithm::RS256,
    }
}

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
    let mut validation = Validation::new(jwt_algorithm(config));
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

    const TEST_RSA_PRIVATE_KEY: &str = r#"-----BEGIN PRIVATE KEY-----
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

    const TEST_RSA_PUBLIC_KEY: &str = r#"-----BEGIN PUBLIC KEY-----
MIIBIjANBgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEA3bmuPUkXUE+bR6a4rxoR
js7Xx9U6K9/WatX5vqDNr/Q5LD9xi7JZRz/FJJVD1kG2UzLbCcqD5T6ox1A3ATE8
hpIgB7yAxIEc++Nc+rjqyZGTR9sMlexBY1oXWg3DWrrFU6H+ZzsjlzmS0OmeVgYO
GMYgRURTzK5NvOIFHcvom5mez4+lRj2weGdKUw/ppEGVF5nm0LKCEuzFIqz9Q2Wt
Gge+WLP3w/bSfF7udScp9xLXMpwy21x3MwTU7xWpF6NZjEJxqM5HpTcCiJiU9PB6
DeAkc1s1IXf+9scJki1G2paZmWNWQVegZdoG7ckHkDjn7Knjx6Vm1HDUstQxquxo
xwIDAQAB
-----END PUBLIC KEY-----"#;

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

    fn rs256_test_config() -> AuthConfig {
        test_config().with_rs256(TEST_RSA_PRIVATE_KEY, TEST_RSA_PUBLIC_KEY)
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
    fn rs256_special_purpose_tokens_round_trip() {
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
            OauthAccessTokenInput {
                app_id: Uuid::new_v4(),
                tenant_id: Uuid::new_v4(),
                role: UserRole::Customer,
                client_id,
                scopes: &scopes,
                grant_type: "client_credentials",
                expires_in_secs: 3600,
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
            },
        )
        .unwrap();

        let claims = decode_access_token(&config, &token).unwrap();
        assert_eq!(claims.exp - claims.iat, 3600);
    }
}
