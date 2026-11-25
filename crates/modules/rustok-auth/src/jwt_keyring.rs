//! Deployment-owned JWT key selection. Only the active key can sign.
use std::borrow::Cow;

use hkdf::Hkdf;
use jsonwebtoken::{DecodingKey, EncodingKey, crypto::aws_lc::DEFAULT_PROVIDER};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::config::{AuthConfig, JwtAlgorithm, hs256_secret_has_min_length, validate_rs256_key_pair};
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
    material: String,
    verify_until: i64,
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

fn valid_kid(kid: &str) -> bool {
    !kid.is_empty()
        && kid.len() <= 64
        && kid.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

impl JwtKeyring {
    pub(crate) fn from_json(value: &str) -> Result<Self> {
        serde_json::from_str(value).map_err(|_| invalid_config())
    }

    pub(crate) fn active_kid(&self) -> &str {
        &self.active_kid
    }

    pub(crate) fn signing_id_is_valid(&self) -> bool {
        valid_kid(&self.active_kid)
            && self.verification_keys.len() <= MAX_VERIFICATION_KEYS
            && !self.verification_keys.iter().any(|key| key.kid == self.active_kid)
    }

    fn with_default_kid(kid: &str) -> Self {
        Self { active_kid: kid.to_string(), ..Self::default() }
    }

    fn explicit_hs256_secrets(&self) -> impl Iterator<Item = &str> {
        self.active_hs256_secret.as_deref().into_iter()
            .chain(self.verification_keys.iter().map(|key| key.material.as_str()))
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
            verification_key(algorithm, &key.material)?;
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

    /// Resolve exactly one configured key. Missing IDs are accepted only by
    /// the explicitly time-bounded initial wire migration; never try keys.
    pub(crate) fn select<'a>(
        &'a self,
        kid: Option<&str>,
        active_material: &'a str,
        now: i64,
    ) -> Option<(&'a str, Option<i64>)> {
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
        if !valid_kid(kid) || !self.signing_id_is_valid() {
            return None;
        }
        let mut matching = self.verification_keys.iter().filter(|key| key.kid == kid);
        let previous = matching.next();
        if matching.next().is_some() {
            return None;
        }
        if kid == self.active_kid {
            return previous.is_none().then_some((active_material, legacy_until));
        }
        let key = previous?;
        let cutoff = legacy_until.map_or(key.verify_until, |until| until.min(key.verify_until));
        (now < cutoff).then_some((key.material.as_str(), Some(cutoff)))
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
struct SpecialJwtKeyring {
    #[serde(default)]
    algorithm: JwtAlgorithm,
    keyring: JwtKeyring,
    #[serde(default)]
    rsa_private_key_pem: Option<String>,
    #[serde(default)]
    rsa_public_key_pem: Option<String>,
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
    password_reset: SpecialJwtKeyring,
    email_verification: SpecialJwtKeyring,
    invite: SpecialJwtKeyring,
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
    pub(crate) fn for_config(config: &'a AuthConfig, purpose: JwtPurpose) -> Self {
        let special = match purpose {
            JwtPurpose::Access => None,
            JwtPurpose::PasswordReset => Some(&config.special_jwt_keyrings.password_reset),
            JwtPurpose::EmailVerification => Some(&config.special_jwt_keyrings.email_verification),
            JwtPurpose::Invite => Some(&config.special_jwt_keyrings.invite),
        };
        let (algorithm, ring, rsa_private, rsa_public) = match special {
            Some(special) => (&special.algorithm, &special.keyring,
                special.rsa_private_key_pem.as_deref(), special.rsa_public_key_pem.as_deref()),
            None => (&config.algorithm, &config.jwt_keyring,
                config.rsa_private_key_pem.as_deref(), config.rsa_public_key_pem.as_deref()),
        };
        Self { algorithm, ring, rsa_private, rsa_public, root: &config.secret, purpose }
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

/// Validate both active and historical key identities across every ring.
/// The old raw host root may only occur as an explicitly retired access key.
pub(crate) fn validate_purpose_keyrings(config: &AuthConfig) -> Result<()> {
    let mut identities = std::collections::HashSet::new();
    let root_identity = key_identity(&JwtAlgorithm::HS256, &config.secret)?;
    for purpose in JwtPurpose::ALL {
        let selected = PurposeKeyringRef::for_config(config, purpose);
        selected.validate()?;
        let active = selected.active_material()?;
        if !identities.insert(key_identity(selected.algorithm, &active)?) {
            return Err(invalid_config());
        }
        for key in &selected.ring.verification_keys {
            let identity = key_identity(selected.algorithm, &key.material)?;
            if (identity == root_identity && purpose != JwtPurpose::Access)
                || !identities.insert(identity)
            {
                return Err(invalid_config());
            }
        }
    }
    Ok(())
}

/// Directly constructed configurations also fail closed if the selected key
/// aliases another key. The scan is bounded and never attempts signatures
/// with non-selected keys. Startup performs the full pair/config validation.
pub(crate) fn ensure_selected_key_isolated(
    config: &AuthConfig,
    expected: JwtPurpose,
    algorithm: &JwtAlgorithm,
    material: &str,
) -> Result<()> {
    let identity = key_identity(algorithm, material)?;
    if expected != JwtPurpose::Access
        && *algorithm == JwtAlgorithm::HS256
        && identity == key_identity(algorithm, &config.secret)?
    {
        return Err(invalid_config());
    }
    let mut matches = 0;
    for purpose in JwtPurpose::ALL {
        let selected = PurposeKeyringRef::for_config(config, purpose);
        if selected.algorithm != algorithm {
            continue;
        }
        if !selected.ring.signing_id_is_valid() {
            return Err(invalid_config());
        }
        let active = selected.active_material()?;
        for candidate in std::iter::once(active.as_ref())
            .chain(selected.ring.verification_keys.iter().map(|key| key.material.as_str()))
        {
            if key_identity(algorithm, candidate)? == identity {
                if purpose != expected {
                    return Err(invalid_config());
                }
                matches += 1;
            }
        }
    }
    if matches != 1 {
        return Err(invalid_config());
    }
    Ok(())
}

/// Host production policy applies to the root and all explicitly supplied
/// symmetric material. Derived keys inherit root strength.
pub(crate) fn explicit_hs256_secrets(config: &AuthConfig) -> impl Iterator<Item = &str> {
    std::iter::once(config.secret.as_str()).chain(
        JwtPurpose::ALL.into_iter().flat_map(move |purpose| {
            let selected = PurposeKeyringRef::for_config(config, purpose);
            (*selected.algorithm == JwtAlgorithm::HS256)
                .then_some(selected.ring)
                .into_iter()
                .flat_map(JwtKeyring::explicit_hs256_secrets)
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
        JwtAlgorithm::HS256 if hs256_secret_has_min_length(material) => {
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

    #[test]
    fn retirement_and_legacy_cutoffs_are_exclusive() {
        let ring = JwtKeyring::from_json(r#"{
            "active_kid":"next",
            "verification_keys":[{"kid":"old","material":"old-material","verify_until":200}],
            "legacy_without_kid":{"kid":"old","verify_until":150}
        }"#).unwrap();
        assert_eq!(ring.select(Some("old"), "new-material", 199), Some(("old-material", Some(200))));
        assert!(ring.select(Some("old"), "new-material", 200).is_none());
        assert_eq!(ring.select(None, "new-material", 149), Some(("old-material", Some(150))));
        assert!(ring.select(None, "new-material", 150).is_none());
        assert_eq!(ring.select(Some("next"), "new-material", 200), Some(("new-material", None)));
    }

    #[test]
    fn effective_hmac_aliases_have_same_identity() {
        let secret = "distinct-material-at-least-32-bytes";
        let padded = format!("{secret}\0\0");
        assert_eq!(key_identity(&JwtAlgorithm::HS256, secret).unwrap(),
            key_identity(&JwtAlgorithm::HS256, &padded).unwrap());
    }
}
