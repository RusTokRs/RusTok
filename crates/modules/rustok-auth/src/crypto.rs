//! Purpose-separated host authentication proofs. JWT key rotation does not
//! change the host MAC root or invalidate credential-bound proofs.
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;

use crate::config::{AuthConfig, mac_root_is_valid};
use crate::error::{AuthError, Result};

/// Closed list: callers cannot supply arbitrary key derivation labels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMacPurpose {
    OAuthConsentCsrf,
    PasswordResetCredential,
    SessionRefreshRetry,
}

impl AuthMacPurpose {
    fn label(self) -> &'static [u8] {
        match self {
            Self::OAuthConsentCsrf => b"oauth-consent-csrf",
            Self::PasswordResetCredential => b"password-reset-credential",
            Self::SessionRefreshRetry => b"session-refresh-retry",
        }
    }
}

fn derive_key(config: &AuthConfig, purpose: AuthMacPurpose) -> Result<[u8; 32]> {
    if !mac_root_is_valid(config.secret()) {
        return Err(AuthError::Internal("Invalid independent Auth MAC root".to_string()));
    }
    let hkdf = Hkdf::<Sha256>::new(
        Some(b"rustok/auth/host-mac"),
        config.secret().as_bytes(),
    );
    let mut key = [0_u8; 32];
    hkdf.expand(purpose.label(), &mut key)
        .map_err(|_| AuthError::Internal("Auth MAC key derivation failed".to_string()))?;
    Ok(key)
}

fn hmac_sha256(key: &[u8], payload: &[u8]) -> Result<[u8; 32]> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key)
        .map_err(|_| AuthError::Internal("Auth MAC initialization failed".to_string()))?;
    mac.update(payload);
    let bytes = mac.finalize().into_bytes();
    let mut tag = [0_u8; 32];
    tag.copy_from_slice(&bytes);
    Ok(tag)
}

/// Canonical lowercase hexadecimal representation of a full SHA-256 MAC.
/// Validate the fixed-size wire representation before comparing a proof.
pub fn is_canonical_auth_mac_tag(value: &str) -> bool {
    value.len() == 64
        && value.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Compute a purpose-scoped proof without exposing the derived key.
/// Verification must use a constant-time comparison.
pub fn auth_purpose_mac(
    config: &AuthConfig,
    purpose: AuthMacPurpose,
    payload: &[u8],
) -> Result<[u8; 32]> {
    let key = derive_key(config, purpose)?;
    hmac_sha256(&key, payload)
}

/// A secret, client-generated 256-bit operation key, not a public request ID.
/// Never log its value. Clients retain the same key only for an ambiguous
/// outcome of the same refresh attempt.
pub const REFRESH_RETRY_KEY_HEADER: &str = "x-rustok-refresh-retry-key";

#[derive(Clone)]
pub struct AuthRefreshRetryKey([u8; 32]);

impl AuthRefreshRetryKey {
    /// Shared singleton-header admission for REST and GraphQL.
    pub fn from_header_values<'a>(
        mut values: impl Iterator<Item = &'a [u8]>,
    ) -> Result<Option<Self>> {
        let Some(raw) = values.next() else {
            return Ok(None);
        };
        let invalid = || AuthError::Validation("Invalid refresh retry key".to_string());
        if values.next().is_some() {
            return Err(invalid());
        }
        let value = std::str::from_utf8(raw).map_err(|_| invalid())?;
        if !is_canonical_auth_mac_tag(value) {
            return Err(invalid());
        }
        let mut key = [0_u8; 32];
        hex::decode_to_slice(value, &mut key).map_err(|_| invalid())?;
        if key == [0_u8; 32] {
            return Err(invalid());
        }
        Ok(Some(Self(key)))
    }

    /// Deterministically derive one opaque candidate for this operation and
    /// session. The host MUST compare its digest with the authoritative
    /// session and history under lock before returning or persisting it.
    /// No credential or operation key is stored in recoverable form.
    pub fn derive_token(
        &self,
        config: &AuthConfig,
        tenant_id: uuid::Uuid,
        session_id: uuid::Uuid,
    ) -> Result<String> {
        if tenant_id.is_nil() || session_id.is_nil() {
            return Err(AuthError::Internal("Invalid refresh session identity".to_string()));
        }
        let mut payload = [0_u8; 64];
        payload[..16].copy_from_slice(tenant_id.as_bytes());
        payload[16..32].copy_from_slice(session_id.as_bytes());
        payload[32..].copy_from_slice(&self.0);
        Ok(hex::encode(auth_purpose_mac(
            config,
            AuthMacPurpose::SessionRefreshRetry,
            &payload,
        )?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_rfc_4231_sha256_vector() {
        let tag = hmac_sha256(&[0x0b; 20], b"Hi There").unwrap();
        assert_eq!(
            hex::encode(tag),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }

    #[test]
    fn host_mac_keys_are_stable_and_purpose_separated() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let config = crate::build_auth_config_from_document(
            "test-secret-at-least-thirty-two-bytes".to_string(), 900, Default::default(),
            fixture.document(1).to_string().as_bytes(),
        ).unwrap().1;
        let csrf = AuthMacPurpose::OAuthConsentCsrf;
        let reset = AuthMacPurpose::PasswordResetCredential;
        assert_ne!(derive_key(&config, csrf).unwrap(), derive_key(&config, reset).unwrap());
        let payload = b"same payload";
        let tag = auth_purpose_mac(&config, csrf, payload).unwrap();
        assert_eq!(tag, auth_purpose_mac(&config.clone(), csrf, payload).unwrap());
        assert_ne!(tag, auth_purpose_mac(&config, reset, payload).unwrap());
        assert_ne!(tag, auth_purpose_mac(&config, csrf, b"other payload").unwrap());
        assert_ne!(tag, hmac_sha256(config.secret().as_bytes(), payload).unwrap());
    }

    #[test]
    fn refresh_retry_key_is_singleton_and_canonical() {
        let key = "ab".repeat(32);
        assert!(AuthRefreshRetryKey::from_header_values(std::iter::empty()).unwrap().is_none());
        assert!(AuthRefreshRetryKey::from_header_values([key.as_bytes()].into_iter()).unwrap().is_some());
        assert!(AuthRefreshRetryKey::from_header_values([key.as_bytes(), key.as_bytes()].into_iter()).is_err());
        for invalid in ["".to_string(), "00".repeat(32), "AB".repeat(32), "a".repeat(63), format!(" {key}")] {
            assert!(AuthRefreshRetryKey::from_header_values([invalid.as_bytes()].into_iter()).is_err());
        }
    }

    #[test]
    fn refresh_retry_derivation_is_stable_and_session_scoped() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let config = crate::build_auth_config_from_document(
            "test-secret-at-least-thirty-two-bytes".to_string(), 900, Default::default(),
            fixture.document(1).to_string().as_bytes(),
        ).unwrap().1;
        let raw = "ab".repeat(32);
        let key = AuthRefreshRetryKey::from_header_values([raw.as_bytes()].into_iter()).unwrap().unwrap();
        let tenant = uuid::Uuid::new_v4();
        let session = uuid::Uuid::new_v4();
        let first = key.derive_token(&config, tenant, session).unwrap();
        assert!(is_canonical_auth_mac_tag(&first));
        assert_eq!(first, key.derive_token(&config.clone(), tenant, session).unwrap());
        assert_ne!(first, key.derive_token(&config, uuid::Uuid::new_v4(), session).unwrap());
        assert_ne!(first, key.derive_token(&config, tenant, uuid::Uuid::new_v4()).unwrap());
    }

    #[test]
    fn mac_tag_requires_full_lowercase_hex() {
        assert!(is_canonical_auth_mac_tag(&"a0".repeat(32)));
        for invalid in ["", "aa", &"A0".repeat(32), &"g0".repeat(32), &"a0".repeat(33)] {
            assert!(!is_canonical_auth_mac_tag(invalid));
        }
    }

    #[test]
    fn invalid_root_fails_closed_for_every_mac_purpose() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let pair = rustok_test_utils::jwt::RsaKeyPairFixture::generate().unwrap();
        for root in ["short", pair.public_pem(), pair.private_pem()] {
            let config = AuthConfig::from_test_document_unvalidated(
                root.to_string(), fixture.document(1).to_string().as_bytes(),
            );
            for purpose in [AuthMacPurpose::OAuthConsentCsrf, AuthMacPurpose::PasswordResetCredential, AuthMacPurpose::SessionRefreshRetry] {
                assert!(auth_purpose_mac(&config, purpose, b"payload").is_err());
            }
        }
    }
}
