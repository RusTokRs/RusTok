//! Cryptographic content digests for Fly integrity gates.
//!
//! `ProjectHash` is a cheap, non-cryptographic change-detection token (FNV-1a 64). It is fine for
//! dirty tracking, ETag-style comparison and optimistic concurrency, but it must never be the only
//! thing standing between a caller and a trusted artefact: a 64-bit non-cryptographic fingerprint
//! can be collided on demand.
//!
//! Every gate that answers "is this payload the one that was approved?" — snapshot restore,
//! project-bundle import, runtime-scenario release baselines — uses [`ContentDigest`] instead.

use crate::{FlyError, FlyResult};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Algorithm label embedded in every serialized [`ContentDigest`].
pub const FLY_DIGEST_ALGORITHM: &str = "sha256";

/// Lowercase hex SHA-256 of `bytes`.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Compare two byte slices without leaking the position of the first difference through timing.
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (left, right) in left.iter().zip(right) {
        difference |= left ^ right;
    }
    difference == 0
}

/// A collision-resistant digest of serialized content, rendered as `sha256:<64 hex chars>`.
///
/// The algorithm prefix is part of the serialized form so that a future algorithm change is an
/// additive, self-describing migration rather than a silent reinterpretation of stored strings.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ContentDigest(String);

impl ContentDigest {
    /// Digest raw bytes.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        Self(format!("{FLY_DIGEST_ALGORITHM}:{}", sha256_hex(bytes)))
    }

    /// Digest the canonical JSON encoding of `value`.
    ///
    /// `serde_json` orders object keys deterministically, so the encoding is stable across runs
    /// and processes for the same logical value.
    pub fn from_json(value: &impl Serialize) -> FlyResult<Self> {
        let bytes = serde_json::to_vec(value).map_err(|error| FlyError::Encode(error.to_string()))?;
        Ok(Self::from_bytes(&bytes))
    }

    /// Parse a previously serialized digest, rejecting unknown algorithms and malformed hex.
    pub fn parse(value: &str) -> Option<Self> {
        let (algorithm, hex) = value.split_once(':')?;
        if algorithm != FLY_DIGEST_ALGORITHM || hex.len() != 64 {
            return None;
        }
        if !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return None;
        }
        Some(Self(value.to_string()))
    }

    /// Full `sha256:<hex>` representation.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Algorithm label without the digest body.
    pub fn algorithm(&self) -> &str {
        self.0.split_once(':').map_or("", |(algorithm, _)| algorithm)
    }

    /// Hex digest body without the algorithm label.
    pub fn hex(&self) -> &str {
        self.0.split_once(':').map_or("", |(_, hex)| hex)
    }

    /// Constant-time equality, for use on verification paths.
    pub fn matches(&self, other: &Self) -> bool {
        constant_time_eq(self.0.as_bytes(), other.0.as_bytes())
    }
}

impl fmt::Display for ContentDigest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn sha256_matches_known_vector() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(
            sha256_hex(b""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn digest_is_self_describing_and_round_trips() {
        let digest = ContentDigest::from_bytes(b"abc");
        assert_eq!(digest.algorithm(), "sha256");
        assert_eq!(digest.hex().len(), 64);
        assert_eq!(
            digest.as_str(),
            "sha256:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        assert_eq!(ContentDigest::parse(digest.as_str()), Some(digest));
    }

    #[test]
    fn parse_rejects_unknown_algorithms_and_malformed_bodies() {
        for value in [
            "md5:ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "sha256:short",
            "sha256:BA7816BF8F01CFEA414140DE5DAE2223B00361A396177A9CB410FF61F20015AD",
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "",
        ] {
            assert_eq!(ContentDigest::parse(value), None, "accepted `{value}`");
        }
    }

    #[test]
    fn json_digest_is_stable_and_order_independent() {
        let left = ContentDigest::from_json(&json!({ "a": 1, "b": [2, 3] })).expect("digest");
        let right = ContentDigest::from_json(&json!({ "b": [2, 3], "a": 1 })).expect("digest");
        assert!(left.matches(&right));
        let different = ContentDigest::from_json(&json!({ "a": 1, "b": [2, 4] })).expect("digest");
        assert!(!left.matches(&different));
    }

    #[test]
    fn constant_time_eq_compares_content_not_length_only() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"abc", b"ab"));
    }
}
