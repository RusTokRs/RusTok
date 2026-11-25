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
}

impl AuthMacPurpose {
    fn label(self) -> &'static [u8] {
        match self {
            Self::OAuthConsentCsrf => b"oauth-consent-csrf",
            Self::PasswordResetCredential => b"password-reset-credential",
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
        let config = AuthConfig::new("test-secret-at-least-thirty-two-bytes".to_string());
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
    fn mac_tag_requires_full_lowercase_hex() {
        assert!(is_canonical_auth_mac_tag(&"a0".repeat(32)));
        for invalid in ["", "aa", &"A0".repeat(32), &"g0".repeat(32), &"a0".repeat(33)] {
            assert!(!is_canonical_auth_mac_tag(invalid));
        }
    }

    #[test]
    fn invalid_root_fails_closed_for_every_mac_purpose() {
        let pair = rustok_test_utils::jwt::RsaKeyPairFixture::generate().unwrap();
        for root in ["short", pair.public_pem(), pair.private_pem()] {
            let config = AuthConfig::new(root.to_string());
            for purpose in [AuthMacPurpose::OAuthConsentCsrf, AuthMacPurpose::PasswordResetCredential] {
                assert!(auth_purpose_mac(&config, purpose, b"payload").is_err());
            }
        }
    }
}
