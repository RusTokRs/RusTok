//! Deployment-owned JWT key selection. Only the active key can sign.
use std::collections::{BTreeMap, HashMap, HashSet};

use jsonwebtoken::{DecodingKey, EncodingKey, crypto::aws_lc::DEFAULT_PROVIDER};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::{JwtKeyInputs, mac_root_is_valid, validate_rs256_key_pair};
use crate::error::{AuthError, Result};
use crate::jwt_fence::{KeyMetadata, RS256_ALGORITHM_MARKER, RingMetadata};

const MAX_VERIFICATION_KEYS: usize = 16;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JwtKeyring {
    active_kid: String,
    #[serde(default)]
    verification_keys: Vec<VerificationKey>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationKey {
    kid: String,
    material: String,
    verify_until: i64,
}

impl std::fmt::Debug for JwtKeyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtKeyring")
            .field("verification_key_count", &self.verification_keys.len())
            .finish_non_exhaustive()
    }
}

pub(crate) fn invalid_config() -> AuthError {
    AuthError::Internal("Invalid JWT keyring configuration".to_string())
}

pub(crate) fn valid_kid(kid: &str) -> bool {
    !kid.is_empty()
        && kid.len() <= 64
        && kid
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl JwtKeyring {
    pub(crate) fn validate(&self, active_material: &str) -> Result<()> {
        if !valid_kid(&self.active_kid) || self.verification_keys.len() > MAX_VERIFICATION_KEYS {
            return Err(invalid_config());
        }
        verification_key(active_material)?;
        for (index, key) in self.verification_keys.iter().enumerate() {
            if !valid_kid(&key.kid)
                || key.kid == self.active_kid
                || key.verify_until <= 0
                || key.material == active_material
                || self.verification_keys[..index]
                    .iter()
                    .any(|other| other.kid == key.kid || other.material == key.material)
            {
                return Err(invalid_config());
            }
            verification_key(&key.material)?;
        }
        Ok(())
    }
}

/// Token purpose is selected by the trusted caller, never from unverified claims.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum JwtPurpose {
    Access,
    PasswordReset,
    EmailVerification,
    Invite,
}

impl JwtPurpose {
    const ALL: [Self; 4] = [
        Self::Access,
        Self::PasswordReset,
        Self::EmailVerification,
        Self::Invite,
    ];

    pub(crate) fn typ(self) -> &'static str {
        match self {
            Self::Access => "JWT",
            Self::PasswordReset => "rustok-reset+jwt",
            Self::EmailVerification => "rustok-email-verification+jwt",
            Self::Invite => "rustok-invite+jwt",
        }
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpecialJwtKeyring {
    pub(crate) keyring: JwtKeyring,
    pub(crate) rsa_private_key_pem: String,
    pub(crate) rsa_public_key_pem: String,
}

/// Independent internal-link signing configuration.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SpecialJwtKeyrings {
    pub(crate) password_reset: SpecialJwtKeyring,
    pub(crate) email_verification: SpecialJwtKeyring,
    pub(crate) invite: SpecialJwtKeyring,
}

impl std::fmt::Debug for SpecialJwtKeyrings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SpecialJwtKeyrings")
            .field("password_reset", &self.password_reset.keyring)
            .field("email_verification", &self.email_verification.keyring)
            .field("invite", &self.invite.keyring)
            .finish()
    }
}

pub(crate) struct PurposeKeyringRef<'a> {
    ring: &'a JwtKeyring,
    rsa_private: &'a str,
    rsa_public: &'a str,
}

impl<'a> PurposeKeyringRef<'a> {
    fn for_inputs(inputs: &'a JwtKeyInputs, purpose: JwtPurpose) -> Self {
        match purpose {
            JwtPurpose::Access => Self {
                ring: &inputs.jwt_keyring,
                rsa_private: &inputs.rsa_private_key_pem,
                rsa_public: &inputs.rsa_public_key_pem,
            },
            JwtPurpose::PasswordReset => Self {
                ring: &inputs.special_jwt_keyrings.password_reset.keyring,
                rsa_private: &inputs
                    .special_jwt_keyrings
                    .password_reset
                    .rsa_private_key_pem,
                rsa_public: &inputs
                    .special_jwt_keyrings
                    .password_reset
                    .rsa_public_key_pem,
            },
            JwtPurpose::EmailVerification => Self {
                ring: &inputs.special_jwt_keyrings.email_verification.keyring,
                rsa_private: &inputs
                    .special_jwt_keyrings
                    .email_verification
                    .rsa_private_key_pem,
                rsa_public: &inputs
                    .special_jwt_keyrings
                    .email_verification
                    .rsa_public_key_pem,
            },
            JwtPurpose::Invite => Self {
                ring: &inputs.special_jwt_keyrings.invite.keyring,
                rsa_private: &inputs.special_jwt_keyrings.invite.rsa_private_key_pem,
                rsa_public: &inputs.special_jwt_keyrings.invite.rsa_public_key_pem,
            },
        }
    }

    fn active_material(&self) -> &'a str {
        self.rsa_public
    }

    fn validate(&self) -> Result<()> {
        self.ring.validate(self.active_material())?;
        validate_rs256_key_pair(self.rsa_private, self.active_material())
    }
}

/// Compare canonical RSA moduli, not PEM/DER representations. Reusing a modulus
/// with a different exponent is also unsafe.
fn key_identity(material: &str) -> Result<[u8; 32]> {
    let key = verification_key(material)?;
    let (modulus, _) = rsa_public_components(&key)?;
    let mut digest = Sha256::new();
    digest.update(b"RS256");
    digest.update(modulus);
    let mut identity = [0_u8; 32];
    identity.copy_from_slice(&digest.finalize());
    Ok(identity)
}

/// Exact verification identity is stricter than the RSA modulus reuse identity.
fn verification_identity(key: &DecodingKey) -> Result<[u8; 32]> {
    let (modulus, exponent) = rsa_public_components(key)?;
    let mut digest = Sha256::new();
    digest.update(b"RS256-verification");
    digest.update((modulus.len() as u64).to_be_bytes());
    digest.update(modulus);
    digest.update((exponent.len() as u64).to_be_bytes());
    digest.update(exponent);
    let mut identity = [0_u8; 32];
    identity.copy_from_slice(&digest.finalize());
    Ok(identity)
}

/// Prepared key material never changes. History contains decoding keys only.
/// The active signer and its verification key are constructed together.
pub(crate) struct ValidatedJwtKeyring {
    active_kid: String,
    signing_key: EncodingKey,
    active_verification_key: DecodingKey,
    active_identity: [u8; 32],
    active_verification_identity: [u8; 32],
    history: HashMap<String, PreparedVerificationKey>,
}

struct PreparedVerificationKey {
    key: DecodingKey,
    identity: [u8; 32],
    verification_identity: [u8; 32],
    verify_until: i64,
}

impl ValidatedJwtKeyring {
    pub(crate) fn active_kid(&self) -> &str {
        &self.active_kid
    }

    pub(crate) fn signing_key(&self) -> &EncodingKey {
        &self.signing_key
    }

    /// Exact ID lookup; retirement remains a live clock check, never cached.
    pub(crate) fn select(&self, kid: &str, now: i64) -> Option<(&DecodingKey, Option<i64>)> {
        if !valid_kid(kid) {
            return None;
        }
        if kid == self.active_kid {
            return Some((&self.active_verification_key, None));
        }
        let previous = self.history.get(kid)?;
        let cutoff = previous.verify_until;
        (now < cutoff).then_some((&previous.key, Some(cutoff)))
    }

    fn prepare(
        inputs: &JwtKeyInputs,
        purpose: JwtPurpose,
        identities: &mut HashSet<[u8; 32]>,
    ) -> Result<Self> {
        let selected = PurposeKeyringRef::for_inputs(inputs, purpose);
        selected.validate()?;
        let active = selected.active_material();
        let active_identity = key_identity(active)?;
        if !identities.insert(active_identity) {
            return Err(invalid_config());
        }
        let signing_key = rsa_signing_key(selected.rsa_private, active)?;
        let active_verification_key = verification_key(active)?;
        let active_verification_identity = verification_identity(&active_verification_key)?;
        let mut history = HashMap::with_capacity(selected.ring.verification_keys.len());
        for previous in &selected.ring.verification_keys {
            let identity = key_identity(&previous.material)?;
            if !identities.insert(identity) {
                return Err(invalid_config());
            }
            let key = verification_key(&previous.material)?;
            let verification_identity = verification_identity(&key)?;
            if history
                .insert(
                    previous.kid.clone(),
                    PreparedVerificationKey {
                        key,
                        identity,
                        verification_identity,
                        verify_until: previous.verify_until,
                    },
                )
                .is_some()
            {
                return Err(invalid_config());
            }
        }
        Ok(Self {
            active_kid: selected.ring.active_kid.clone(),
            signing_key,
            active_verification_key,
            active_identity,
            active_verification_identity,
            history,
        })
    }
}

/// All purposes are validated atomically before any purpose can be used.
/// There is no public constructor for an individual prepared ring.
pub(crate) struct ValidatedJwtKeyrings {
    access: ValidatedJwtKeyring,
    password_reset: ValidatedJwtKeyring,
    email_verification: ValidatedJwtKeyring,
    invite: ValidatedJwtKeyring,
}

impl ValidatedJwtKeyrings {
    pub(crate) fn prepare(inputs: &JwtKeyInputs, mac_root: &str) -> Result<Self> {
        if !mac_root_is_valid(mac_root) {
            return Err(invalid_config());
        }
        let mut identities = HashSet::new();
        let mut prepare = |purpose| ValidatedJwtKeyring::prepare(inputs, purpose, &mut identities);
        Ok(Self {
            access: prepare(JwtPurpose::Access)?,
            password_reset: prepare(JwtPurpose::PasswordReset)?,
            email_verification: prepare(JwtPurpose::EmailVerification)?,
            invite: prepare(JwtPurpose::Invite)?,
        })
    }

    /// Expose only canonical metadata with keyed identities to durable admission.
    pub(crate) fn fence_snapshot(
        &self,
        fingerprint: impl Fn(&[u8; 32]) -> Result<String>,
    ) -> Result<crate::jwt_fence::Snapshot> {
        let mut result = BTreeMap::new();
        for purpose in JwtPurpose::ALL {
            let ring = self.for_purpose(purpose);
            let mut keys = BTreeMap::new();
            keys.insert(
                ring.active_kid.clone(),
                KeyMetadata {
                    identity: fingerprint(&ring.active_identity)?,
                    verification_identity: fingerprint(&ring.active_verification_identity)?,
                    verify_until: None,
                    algorithm: RS256_ALGORITHM_MARKER.to_string(),
                },
            );
            for (kid, history) in &ring.history {
                keys.insert(
                    kid.clone(),
                    KeyMetadata {
                        identity: fingerprint(&history.identity)?,
                        verification_identity: fingerprint(&history.verification_identity)?,
                        verify_until: Some(history.verify_until),
                        algorithm: RS256_ALGORITHM_MARKER.to_string(),
                    },
                );
            }
            result.insert(
                purpose.typ().to_string(),
                RingMetadata {
                    algorithm: RS256_ALGORITHM_MARKER.to_string(),
                    keys,
                },
            );
        }
        Ok(result)
    }

    pub(crate) fn for_purpose(&self, purpose: JwtPurpose) -> &ValidatedJwtKeyring {
        match purpose {
            JwtPurpose::Access => &self.access,
            JwtPurpose::PasswordReset => &self.password_reset,
            JwtPurpose::EmailVerification => &self.email_verification,
            JwtPurpose::Invite => &self.invite,
        }
    }
}

/// Use the same crypto backend as the JWT verifier; no local ASN.1 parser.
/// AWS-LC returns the unsigned big-endian RSA components.
fn rsa_public_components(key: &DecodingKey) -> Result<(Vec<u8>, Vec<u8>)> {
    let der = key.try_get_as_bytes().map_err(|_| invalid_config())?;
    let (modulus, exponent) = (DEFAULT_PROVIDER
        .key_utils
        .rsa_pub_components_from_public_key)(der)
    .map_err(|_| invalid_config())?;
    let first = modulus.first().copied().ok_or_else(invalid_config)?;
    let bits = modulus
        .len()
        .saturating_mul(8)
        .saturating_sub(first.leading_zeros() as usize);
    if first == 0 || bits < 2048 || exponent.is_empty() {
        return Err(invalid_config());
    }
    Ok((modulus, exponent))
}

/// Verify the private key belongs to the exact public key selected for
/// isolation, including configurations constructed without the startup builder.
pub(crate) fn rsa_signing_key(private_pem: &str, public_pem: &str) -> Result<EncodingKey> {
    let pem = private_pem.trim();
    if !(pem.starts_with("-----BEGIN PRIVATE KEY-----")
        || pem.starts_with("-----BEGIN RSA PRIVATE KEY-----"))
    {
        return Err(invalid_config());
    }
    let encoding = EncodingKey::from_rsa_pem(pem.as_bytes()).map_err(|_| invalid_config())?;
    let decoding = verification_key(public_pem)?;
    let public = rsa_public_components(&decoding)?;
    let from_private = (DEFAULT_PROVIDER
        .key_utils
        .rsa_pub_components_from_private_key)(encoding.as_bytes())
    .map_err(|_| invalid_config())?;
    if from_private != public {
        return Err(invalid_config());
    }
    Ok(encoding)
}

pub(crate) fn verification_key(material: &str) -> Result<DecodingKey> {
    let pem = material.trim();
    if !(pem.starts_with("-----BEGIN PUBLIC KEY-----")
        || pem.starts_with("-----BEGIN RSA PUBLIC KEY-----")
        || pem.starts_with("-----BEGIN CERTIFICATE-----"))
        || pem.contains("PRIVATE KEY-----")
    {
        return Err(invalid_config());
    }
    let key = DecodingKey::from_rsa_pem(pem.as_bytes()).map_err(|_| invalid_config())?;
    rsa_public_components(&key)?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retirement_cutoff_is_exclusive() {
        let fixture = rustok_test_utils::jwt::JwtKeyringFixture::generate().unwrap();
        let old = rustok_test_utils::jwt::RsaKeyPairFixture::generate().unwrap();
        let doc =
            fixture.document_with_access(1, "next", &fixture.access, Some(("old", &old, 200)));
        let config = crate::build_auth_config_from_document(
            "root-test-material-with-at-least-32-bytes".into(),
            900,
            Default::default(),
            doc.to_string().as_bytes(),
        )
        .unwrap()
        .1;
        let ring = config
            .prepared_jwt()
            .unwrap()
            .for_purpose(JwtPurpose::Access);
        assert_eq!(
            ring.select("old", 199).map(|(_, cutoff)| cutoff),
            Some(Some(200))
        );
        assert!(ring.select("old", 200).is_none());
        assert!(ring.select("", 149).is_none());
        assert!(ring.select("unknown", 149).is_none());
        assert_eq!(
            ring.select("next", 200).map(|(_, cutoff)| cutoff),
            Some(None)
        );
    }

    #[test]
    fn rsa_identity_uses_canonical_modulus_not_pem_formatting() {
        let pair = rustok_test_utils::jwt::RsaKeyPairFixture::generate().unwrap();
        let padded = format!(" \n{}\n", pair.public_pem());
        assert_eq!(
            key_identity(pair.public_pem()).unwrap(),
            key_identity(&padded).unwrap()
        );
    }

    #[test]
    fn symmetric_material_is_never_a_verification_key() {
        assert!(verification_key("not-an-rsa-public-key-and-not-a-jwt-secret").is_err());
    }
}
