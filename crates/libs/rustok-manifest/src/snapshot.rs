use crate::error::{ManifestError, Result};
use crate::types::ModulesManifest;

#[cfg(feature = "snapshot-hash")]
use sha2::{Digest, Sha256};

/// Computes a deterministic SHA-256 snapshot hash for the given ModulesManifest.
///
/// Keys are serialized with canonical sorted ordering (via BTreeMap).
#[cfg(feature = "snapshot-hash")]
pub fn compute_modules_manifest_hash(manifest: &ModulesManifest) -> Result<String> {
    let serialized = toml::to_string(manifest).map_err(ManifestError::TomlSerialize)?;
    let mut hasher = Sha256::new();
    hasher.update(serialized.as_bytes());
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

#[cfg(not(feature = "snapshot-hash"))]
pub fn compute_modules_manifest_hash(_manifest: &ModulesManifest) -> Result<String> {
    Err(ManifestError::Validation(
        "snapshot-hash feature is disabled in rustok-manifest".to_string(),
    ))
}
