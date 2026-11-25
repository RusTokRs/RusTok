//! Deployment-owned JWT key selection. Only the active key can sign.
use chrono::Utc;
use jsonwebtoken::DecodingKey;
use serde::Deserialize;

use crate::config::{JwtAlgorithm, hs256_secret_has_min_length};
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

fn invalid_config() -> AuthError {
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

    pub(crate) fn active_hs256_secret<'a>(&'a self, root: &'a str) -> &'a str {
        self.active_hs256_secret.as_deref().unwrap_or(root)
    }

    /// Used by host deployment validation; do not log these values.
    pub fn hs256_secrets<'a>(&'a self, root: &'a str) -> impl Iterator<Item = &'a str> {
        std::iter::once(self.active_hs256_secret(root))
            .chain(self.verification_keys.iter().map(|key| key.material.as_str()))
    }

    pub(crate) fn validate(
        &self,
        algorithm: &JwtAlgorithm,
        root: &str,
        active_public_key: Option<&str>,
    ) -> Result<()> {
        if !valid_kid(&self.active_kid)
            || self.verification_keys.len() > MAX_VERIFICATION_KEYS
            || (*algorithm == JwtAlgorithm::RS256 && self.active_hs256_secret.is_some())
        {
            return Err(invalid_config());
        }
        let active_material = match algorithm {
            JwtAlgorithm::HS256 => self.active_hs256_secret(root),
            JwtAlgorithm::RS256 => active_public_key.ok_or_else(invalid_config)?,
        };
        if *algorithm == JwtAlgorithm::HS256 && !hs256_secret_has_min_length(active_material) {
            return Err(invalid_config());
        }
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
    ) -> Option<&'a str> {
        let now = Utc::now().timestamp();
        let kid = match kid {
            Some(kid) => kid,
            None => {
                let legacy = self.legacy_without_kid.as_ref()?;
                if now >= legacy.verify_until {
                    return None;
                }
                &legacy.kid
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
            return previous.is_none().then_some(active_material);
        }
        let key = previous?;
        (now < key.verify_until).then_some(key.material.as_str())
    }
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
            DecodingKey::from_rsa_pem(pem.as_bytes()).map_err(|_| invalid_config())
        }
    }
}
