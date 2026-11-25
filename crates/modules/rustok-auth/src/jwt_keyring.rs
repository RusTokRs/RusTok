//! Deployment-owned JWT key selection. Only the active key can sign.
use std::{borrow::Cow, collections::{BTreeMap, HashMap, HashSet}};

use hkdf::Hkdf;
use jsonwebtoken::{DecodingKey, EncodingKey, crypto::aws_lc::DEFAULT_PROVIDER};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::{JwtAlgorithm, JwtKeyInputs, hs256_secret_has_min_length, validate_rs256_key_pair};
use crate::error::{AuthError, Result};

const MAX_VERIFICATION_KEYS: usize = 16;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JwtKeyring {
    active_kid: String,
    #[serde(default)]
    active_hs256_secret: Option<String>,
    #[serde(default)]
    verification_keys: Vec<VerificationKey>,
    #[serde(default)]
    legacy_without_kid: Option<LegacyKey>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct VerificationKey {
    kid: String,
    /// Omitted means the active ring algorithm (legacy homogeneous format).
    #[serde(default)]
    algorithm: Option<JwtAlgorithm>,
    material: String,
    verify_until: i64,
}

impl VerificationKey {
    fn algorithm<'a>(&'a self, active: &'a JwtAlgorithm) -> &'a JwtAlgorithm {
        self.algorithm.as_ref().unwrap_or(active)
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct LegacyKey {
    kid: String,
    verify_until: i64,
}

impl Default for JwtKeyring {
    fn default() -> Self {
        Self {
            active_kid: "default".to_string(),
            active_hs256_secret: None,
            verification_keys: Vec::new(),
            legacy_without_kid: None,
        }
    }
}

impl std::fmt::Debug for JwtKeyring {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("JwtKeyring")
            .field("verification_key_count", &self.verification_keys.len())
            .field("legacy_without_kid", &self.legacy_without_kid.is_some())
            .finish_non_exhaustive()
    }
}

pub(crate) fn invalid_config() -> AuthError {
    AuthError::Internal("Invalid JWT keyring configuration".to_string())
}

pub(crate) fn valid_kid(kid: &str) -> bool {
    !kid.is_empty()
        && kid.len() <= 64
        && kid.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl JwtKeyring {
    pub(crate) fn from_json(value: &str) -> Result<Self> {
        serde_json::from_str(value).map_err(|_| invalid_config())
    }

    fn with_default_kid(kid: &str) -> Self {
        Self { active_kid: kid.to_string(), ..Self::default() }
    }

    fn explicit_hs256_secrets<'a>(&'a self, active: &'a JwtAlgorithm) -> impl Iterator<Item = &'a str> {
        self.active_hs256_secret.as_deref().into_iter().chain(
            self.verification_keys.iter()
                .filter(move |key| *key.algorithm(active) == JwtAlgorithm::HS256)
                .map(|key| key.material.as_str()),
        )
    }

    pub(crate) fn validate(
        &self,
        algorithm: &JwtAlgorithm,
        active_material: &str,
        allow_missing_kid: bool,
    ) -> Result<()> {
        if !valid_kid(&self.active_kid)
            || self.verification_keys.len() > MAX_VERIFICATION_KEYS
            || (*algorithm == JwtAlgorithm::RS256 && self.active_hs256_secret.is_some())
            || (!allow_missing_kid && self.legacy_without_kid.is_some())
        {
            return Err(invalid_config());
        }
        verification_key(algorithm, active_material)?;
        for (index, key) in self.verification_keys.iter().enumerate() {
            if !valid_kid(&key.kid)
                || key.kid == self.active_kid
                || key.verify_until <= 0
                || key.material == active_material
                || self.verification_keys[..index].iter().any(|other| {
                    other.kid == key.kid || other.material == key.material
                })
            {
                return Err(invalid_config());
            }
            verification_key(key.algorithm(algorithm), &key.material)?;
        }
        if let Some(legacy) = &self.legacy_without_kid {
            if !valid_kid(&legacy.kid) || legacy.verify_until <= 0 {
                return Err(invalid_config());
            }
            if legacy.kid != self.active_kid
                && !self.verification_keys.iter().any(|key| {
                    key.kid == legacy.kid && legacy.verify_until <= key.verify_until
                })
            {
                return Err(invalid_config());
            }
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
    const ALL: [Self; 4] = [Self::Access, Self::PasswordReset, Self::EmailVerification, Self::Invite];

    pub(crate) fn typ(self) -> &'static str {
        match self {
            Self::Access => "JWT",
            Self::PasswordReset => "rustok-reset+jwt",
            Self::EmailVerification => "rustok-email-verification+jwt",
            Self::Invite => "rustok-invite+jwt",
        }
    }

    fn label(self) -> &'static [u8] {
        match self {
            Self::Access => b"access",
            Self::PasswordReset => b"password-reset",
            Self::EmailVerification => b"email-verification",
            Self::Invite => b"invite",
        }
    }

    /// Defaults use independent derived signing keys, never the raw host MAC root.
    fn derive_hs256(self, root: &str) -> Result<String> {
        if !hs256_secret_has_min_length(root) {
            return Err(invalid_config());
        }
        let hkdf = Hkdf::<Sha256>::new(Some(b"rustok/auth/jwt-purpose/v1"), root.as_bytes());
        let mut key = [0_u8; 32];
        hkdf.expand(self.label(), &mut key).map_err(|_| invalid_config())?;
        Ok(hex::encode(key))
    }
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SpecialJwtKeyring {
    #[serde(default)]
    pub(crate) algorithm: JwtAlgorithm,
    pub(crate) keyring: JwtKeyring,
    #[serde(default)]
    pub(crate) rsa_private_key_pem: Option<String>,
    #[serde(default)]
    pub(crate) rsa_public_key_pem: Option<String>,
}

impl SpecialJwtKeyring {
    fn default_for(kid: &str) -> Self {
        Self {
            algorithm: JwtAlgorithm::HS256,
            keyring: JwtKeyring::with_default_kid(kid),
            rsa_private_key_pem: None,
            rsa_public_key_pem: None,
        }
    }
}

/// Independent internal-link signing configuration. The access ring retains
/// its existing host configuration entry point.
#[derive(Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SpecialJwtKeyrings {
    pub(crate) password_reset: SpecialJwtKeyring,
    pub(crate) email_verification: SpecialJwtKeyring,
    pub(crate) invite: SpecialJwtKeyring,
}

impl Default for SpecialJwtKeyrings {
    fn default() -> Self {
        Self {
            password_reset: SpecialJwtKeyring::default_for("reset-default"),
            email_verification: SpecialJwtKeyring::default_for("verify-default"),
            invite: SpecialJwtKeyring::default_for("invite-default"),
        }
    }
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

impl SpecialJwtKeyrings {
    pub(crate) fn from_json(value: &str) -> Result<Self> {
        serde_json::from_str(value).map_err(|_| invalid_config())
    }
}

pub(crate) struct PurposeKeyringRef<'a> {
    pub(crate) algorithm: &'a JwtAlgorithm,
    pub(crate) ring: &'a JwtKeyring,
    pub(crate) rsa_private: Option<&'a str>,
    rsa_public: Option<&'a str>,
    root: &'a str,
    purpose: JwtPurpose,
}

impl<'a> PurposeKeyringRef<'a> {
    #[cfg(test)]
    pub(crate) fn for_config(config: &'a crate::config::AuthConfig, purpose: JwtPurpose) -> Self {
        Self::for_inputs(config.fixed_inputs().unwrap(), config.secret(), purpose)
    }

    fn for_inputs(inputs: &'a JwtKeyInputs, root: &'a str, purpose: JwtPurpose) -> Self {
        let special = match purpose {
            JwtPurpose::Access => None,
            JwtPurpose::PasswordReset => Some(&inputs.special_jwt_keyrings.password_reset),
            JwtPurpose::EmailVerification => Some(&inputs.special_jwt_keyrings.email_verification),
            JwtPurpose::Invite => Some(&inputs.special_jwt_keyrings.invite),
        };
        let (algorithm, ring, rsa_private, rsa_public) = match special {
            Some(special) => (&special.algorithm, &special.keyring,
                special.rsa_private_key_pem.as_deref(), special.rsa_public_key_pem.as_deref()),
            None => (&inputs.algorithm, &inputs.jwt_keyring,
                inputs.rsa_private_key_pem.as_deref(), inputs.rsa_public_key_pem.as_deref()),
        };
        Self { algorithm, ring, rsa_private, rsa_public, root, purpose }
    }

    pub(crate) fn active_material(&self) -> Result<Cow<'a, str>> {
        match self.algorithm {
            JwtAlgorithm::HS256 => {
                let value = match self.ring.active_hs256_secret.as_deref() {
                    Some(secret) => Cow::Borrowed(secret),
                    None => Cow::Owned(self.purpose.derive_hs256(self.root)?),
                };
                if !hs256_secret_has_min_length(&value)
                    || hmac_identity(value.as_bytes()) == hmac_identity(self.root.as_bytes())
                {
                    return Err(invalid_config());
                }
                Ok(value)
            }
            JwtAlgorithm::RS256 => self.rsa_public.map(Cow::Borrowed).ok_or_else(invalid_config),
        }
    }

    pub(crate) fn validate(&self) -> Result<()> {
        let active = self.active_material()?;
        self.ring.validate(self.algorithm, &active, self.purpose == JwtPurpose::Access)?;
        match self.algorithm {
            JwtAlgorithm::HS256 => {
                if self.rsa_private.is_some() || self.rsa_public.is_some() {
                    return Err(invalid_config());
                }
            }
            JwtAlgorithm::RS256 => {
                validate_rs256_key_pair(self.rsa_private.ok_or_else(invalid_config)?, &active)?;
            }
        }
        Ok(())
    }
}

// Compare HMAC's effective block-sized key, not textual representations.
// For RSA, compare canonical public moduli rather than PEM/DER representations.
// Reusing a modulus with a different exponent is also unsafe.
fn hmac_identity(material: &[u8]) -> [u8; 64] {
    let mut block = [0_u8; 64];
    if material.len() > block.len() {
        block[..32].copy_from_slice(&Sha256::digest(material));
    } else {
        block[..material.len()].copy_from_slice(material);
    }
    block
}

fn key_identity(algorithm: &JwtAlgorithm, material: &str) -> Result<[u8; 32]> {
    let mut digest = Sha256::new();
    match algorithm {
        JwtAlgorithm::HS256 => {
            digest.update(b"HS256");
            digest.update(hmac_identity(material.as_bytes()));
        }
        JwtAlgorithm::RS256 => {
            digest.update(b"RS256");
            let key = verification_key(algorithm, material)?;
            let (modulus, _) = rsa_public_components(&key)?;
            digest.update(modulus);
        }
    }
    let mut identity = [0_u8; 32];
    identity.copy_from_slice(&digest.finalize());
    Ok(identity)
}

/// Exact verification identity is stricter than the RSA modulus reuse identity.
fn verification_identity(
    algorithm: &JwtAlgorithm,
    key: &DecodingKey,
    reuse_identity: [u8; 32],
) -> Result<[u8; 32]> {
    if *algorithm == JwtAlgorithm::HS256 {
        return Ok(reuse_identity);
    }
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
    algorithm: JwtAlgorithm,
    active_kid: String,
    signing_key: EncodingKey,
    active_verification_key: DecodingKey,
    active_identity: [u8; 32],
    active_verification_identity: [u8; 32],
    history: HashMap<String, PreparedVerificationKey>,
    legacy_without_kid: Option<LegacyKey>,
}

struct PreparedVerificationKey {
    algorithm: JwtAlgorithm,
    key: DecodingKey,
    identity: [u8; 32],
    verification_identity: [u8; 32],
    verify_until: i64,
}

impl ValidatedJwtKeyring {
    pub(crate) fn algorithm(&self) -> &JwtAlgorithm {
        &self.algorithm
    }

    pub(crate) fn active_kid(&self) -> &str {
        &self.active_kid
    }

    pub(crate) fn signing_key(&self) -> &EncodingKey {
        &self.signing_key
    }

    /// Exact ID lookup; retirement remains a live clock check, never cached.
    pub(crate) fn select(
        &self, kid: Option<&str>, now: i64,
    ) -> Option<(&JwtAlgorithm, &DecodingKey, Option<i64>)> {
        let (kid, legacy_until) = match kid {
            Some(kid) => (kid, None),
            None => {
                let legacy = self.legacy_without_kid.as_ref()?;
                if now >= legacy.verify_until {
                    return None;
                }
                (legacy.kid.as_str(), Some(legacy.verify_until))
            }
        };
        if !valid_kid(kid) {
            return None;
        }
        if kid == self.active_kid {
            return Some((&self.algorithm, &self.active_verification_key, legacy_until));
        }
        let previous = self.history.get(kid)?;
        let cutoff = legacy_until.map_or(previous.verify_until, |until| until.min(previous.verify_until));
        (now < cutoff).then_some((&previous.algorithm, &previous.key, Some(cutoff)))
    }

    fn prepare(
        inputs: &JwtKeyInputs,
        root: &str,
        purpose: JwtPurpose,
        identities: &mut HashSet<[u8; 32]>,
        root_identity: [u8; 32],
    ) -> Result<Self> {
        let selected = PurposeKeyringRef::for_inputs(inputs, root, purpose);
        selected.validate()?;
        let active = selected.active_material()?;
        let active_identity = key_identity(selected.algorithm, &active)?;
        if !identities.insert(active_identity) {
            return Err(invalid_config());
        }
        let signing_key = match selected.algorithm {
            JwtAlgorithm::HS256 => EncodingKey::from_secret(active.as_bytes()),
            JwtAlgorithm::RS256 => rsa_signing_key(
                selected.rsa_private.ok_or_else(invalid_config)?, &active,
            )?,
        };
        let active_verification_key = verification_key(selected.algorithm, &active)?;
        let active_verification_identity = verification_identity(
            selected.algorithm, &active_verification_key, active_identity,
        )?;
        let mut history = HashMap::with_capacity(selected.ring.verification_keys.len());
        for previous in &selected.ring.verification_keys {
            let algorithm = previous.algorithm(selected.algorithm);
            let identity = key_identity(algorithm, &previous.material)?;
            if (identity == root_identity && purpose != JwtPurpose::Access)
                || !identities.insert(identity)
            {
                return Err(invalid_config());
            }
            let key = verification_key(algorithm, &previous.material)?;
            let verification_identity = verification_identity(algorithm, &key, identity)?;
            if history.insert(previous.kid.clone(), PreparedVerificationKey {
                algorithm: algorithm.clone(), key, identity, verification_identity,
                verify_until: previous.verify_until,
            }).is_some() {
                return Err(invalid_config());
            }
        }
        Ok(Self {
            algorithm: selected.algorithm.clone(),
            active_kid: selected.ring.active_kid.clone(),
            signing_key,
            active_verification_key,
            active_identity,
            active_verification_identity,
            history,
            legacy_without_kid: selected.ring.legacy_without_kid.clone(),
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
    pub(crate) fn prepare(inputs: &JwtKeyInputs, root: &str) -> Result<Self> {
        if !hs256_secret_has_min_length(root) {
            return Err(invalid_config());
        }
        let root_identity = key_identity(&JwtAlgorithm::HS256, root)?;
        let mut identities = HashSet::new();
        let mut prepare = |purpose| ValidatedJwtKeyring::prepare(
            inputs, root, purpose, &mut identities, root_identity,
        );
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
        use crate::jwt_fence::{KeyMetadata, LegacyMetadata, RingMetadata};
        let mut result = BTreeMap::new();
        for purpose in JwtPurpose::ALL {
            let ring = self.for_purpose(purpose);
            let mut keys = BTreeMap::new();
            keys.insert(ring.active_kid.clone(), KeyMetadata {
                identity: fingerprint(&ring.active_identity)?,
                verification_identity: fingerprint(&ring.active_verification_identity)?,
                verify_until: None,
                algorithm: ring.algorithm.clone(),
            });
            for (kid, history) in &ring.history {
                keys.insert(kid.clone(), KeyMetadata {
                    identity: fingerprint(&history.identity)?,
                    verification_identity: fingerprint(&history.verification_identity)?,
                    verify_until: Some(history.verify_until),
                    algorithm: history.algorithm.clone(),
                });
            }
            let legacy = ring.legacy_without_kid.as_ref().map(|legacy| LegacyMetadata {
                kid: legacy.kid.clone(), verify_until: legacy.verify_until,
            });
            result.insert(purpose.typ().to_string(), RingMetadata {
                algorithm: ring.algorithm.clone(), keys, legacy,
            });
        }
        Ok(result)
    }

    /// Preserve identity across adjacent unmanaged generations. Active
    /// algorithm migration requires the durable fence and its tombstones;
    /// fixed configs may still verify a mixed-algorithm history.
    pub(crate) fn validate_successor(&self, next: &Self) -> Result<()> {
        let now = chrono::Utc::now().timestamp();
        for purpose in JwtPurpose::ALL {
            let old = self.for_purpose(purpose);
            let new = next.for_purpose(purpose);
            if old.algorithm != new.algorithm {
                return Err(invalid_config());
            }
            let old_keys = std::iter::once((
                old.active_kid.as_str(), old.active_identity, old.active_verification_identity, None,
            )).chain(old.history.iter().map(|(kid, key)| {
                (kid.as_str(), key.identity, key.verification_identity, Some(key.verify_until))
            }));
            for (kid, identity, verification, cutoff) in old_keys {
                for target in JwtPurpose::ALL {
                    let ring = next.for_purpose(target);
                    let new_keys = std::iter::once((
                        ring.active_kid.as_str(), ring.active_identity, ring.active_verification_identity, None,
                    )).chain(ring.history.iter().map(|(kid, key)| {
                        (kid.as_str(), key.identity, key.verification_identity, Some(key.verify_until))
                    }));
                    for (new_kid, new_identity, new_verification, new_cutoff) in new_keys {
                        if target == purpose && kid == new_kid {
                            if identity != new_identity || verification != new_verification {
                                return Err(invalid_config());
                            }
                            if cutoff.is_some_and(|until| until <= now)
                                && new_cutoff.is_none_or(|until| until > now)
                            {
                                return Err(invalid_config());
                            }
                        }
                        if identity == new_identity && (target != purpose || kid != new_kid) {
                            return Err(invalid_config());
                        }
                    }
                }
            }
        }
        Ok(())
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

/// Host production policy applies to the root and all explicitly supplied
/// symmetric material. Derived keys inherit root strength.
pub(crate) fn explicit_hs256_secrets<'a>(inputs: &'a JwtKeyInputs, root: &'a str) -> impl Iterator<Item = &'a str> {
    std::iter::once(root).chain(
        JwtPurpose::ALL.into_iter().flat_map(move |purpose| {
            let selected = PurposeKeyringRef::for_inputs(inputs, root, purpose);
            selected.ring.explicit_hs256_secrets(selected.algorithm)
        }),
    )
}

/// Use the same crypto backend as the JWT verifier; no local ASN.1 parser.
/// AWS-LC returns the unsigned big-endian RSA components.
fn rsa_public_components(key: &DecodingKey) -> Result<(Vec<u8>, Vec<u8>)> {
    let der = key.try_get_as_bytes().map_err(|_| invalid_config())?;
    let (modulus, exponent) = (DEFAULT_PROVIDER.key_utils.rsa_pub_components_from_public_key)(der)
        .map_err(|_| invalid_config())?;
    // RS256 requires a modulus of at least 2048 bits (RFC 7518).
    let first = modulus.first().copied().ok_or_else(invalid_config)?;
    let bits = modulus.len().saturating_mul(8).saturating_sub(first.leading_zeros() as usize);
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
    let decoding = verification_key(&JwtAlgorithm::RS256, public_pem)?;
    let public = rsa_public_components(&decoding)?;
    let from_private = (DEFAULT_PROVIDER.key_utils.rsa_pub_components_from_private_key)(
        encoding.as_bytes(),
    ).map_err(|_| invalid_config())?;
    if from_private != public {
        return Err(invalid_config());
    }
    Ok(encoding)
}

pub(crate) fn verification_key(algorithm: &JwtAlgorithm, material: &str) -> Result<DecodingKey> {
    match algorithm {
        // An RSA public PEM is public information, never an HMAC secret.
        // Reject PEM-shaped material even if it meets the length floor.
        JwtAlgorithm::HS256 if hs256_secret_has_min_length(material)
            && !material.contains("-----BEGIN ") => {
            Ok(DecodingKey::from_secret(material.as_bytes()))
        }
        JwtAlgorithm::HS256 => Err(invalid_config()),
        JwtAlgorithm::RS256 => {
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthConfig;

    #[test]
    fn retirement_and_legacy_cutoffs_are_exclusive() {
        let mut config = AuthConfig::new("root-test-material-with-at-least-32-bytes".into());
        config.set_test_jwt_keyring(JwtKeyring::from_json(r#"{
            "active_kid":"next",
            "verification_keys":[{"kid":"old","material":"old-verification-material-at-least-32-bytes","verify_until":200}],
            "legacy_without_kid":{"kid":"old","verify_until":150}
        }"#).unwrap());
        let ring = config.prepared_jwt().unwrap().for_purpose(JwtPurpose::Access);
        assert_eq!(ring.select(Some("old"), 199).map(|(_, _, cutoff)| cutoff), Some(Some(200)));
        assert!(ring.select(Some("old"), 200).is_none());
        assert_eq!(ring.select(None, 149).map(|(_, _, cutoff)| cutoff), Some(Some(150)));
        assert!(ring.select(None, 150).is_none());
        assert_eq!(ring.select(Some("next"), 200).map(|(_, _, cutoff)| cutoff), Some(None));
    }

    #[test]
    fn public_pem_cannot_be_configured_as_hmac_material() {
        let mut config = AuthConfig::new("root-test-material-with-at-least-32-bytes".into());
        config.set_test_jwt_keyring(JwtKeyring::from_json(&serde_json::json!({
            "active_kid": "hs",
            "verification_keys": [{"kid":"wrong","algorithm":"HS256",
                "material":crate::jwt::TEST_RSA_PUBLIC_KEY,"verify_until":200}]
        }).to_string()).unwrap());
        assert!(crate::validate_auth_config(&config).is_err());
    }

    #[test]
    fn effective_hmac_aliases_have_same_identity() {
        let secret = "distinct-material-at-least-32-bytes";
        let padded = format!("{secret}\0\0");
        assert_eq!(key_identity(&JwtAlgorithm::HS256, secret).unwrap(),
            key_identity(&JwtAlgorithm::HS256, &padded).unwrap());
    }
}
