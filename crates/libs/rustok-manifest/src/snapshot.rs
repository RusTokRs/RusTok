use crate::error::{ManifestError, Result};
use crate::types::ModulesManifest;

#[cfg(feature = "snapshot-hash")]
use sha2::{Digest, Sha256};

/// Computes a deterministic SHA-256 snapshot hash for the given ModulesManifest.
///
/// Keys are serialized with canonical sorted ordering (via BTreeMap).
#[cfg(feature = "snapshot-hash")]
pub fn compute_modules_manifest_hash(manifest: &ModulesManifest) -> Result<String> {
    let json = serde_json::to_vec(manifest).map_err(|e| {
        ManifestError::Validation(format!("JSON serialization error for hash: {e}"))
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&json);
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(not(feature = "snapshot-hash"))]
pub fn compute_modules_manifest_hash(_manifest: &ModulesManifest) -> Result<String> {
    Err(ManifestError::Validation(
        "snapshot-hash feature is disabled in rustok-manifest".to_string(),
    ))
}
