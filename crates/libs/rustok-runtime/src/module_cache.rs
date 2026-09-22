//! Digest-verified node-local module payload cache.
//!
//! The helpers in this module own only local files beneath an already prepared
//! instance root. They neither select an artifact nor contact CAS, a registry,
//! a database, or a sandbox. An operations agent supplies owner-issued bytes
//! and retains responsibility for reporting readiness to its control plane.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{InstanceLayout, InstanceLayoutError};

/// Exact local receipt for a digest-addressed module payload cache entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModulePayloadMaterializationReceipt {
    pub payload_digest: String,
    pub payload_path: String,
    pub size_bytes: u64,
    /// `true` means a pre-existing regular cache file was rehashed and reused.
    pub resumed: bool,
}

/// Exact local receipt for a prepared artifact runtime cache entry. It is not
/// owner evidence: the agent still derives and reports its own immutable
/// health evidence through the authenticated controller port.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreparedModuleCacheReceipt {
    pub runtime_fingerprint: String,
    pub payload_digest: String,
    pub payload_path: String,
    pub preparation_path: String,
    /// `true` means the exact prior local preparation marker was revalidated.
    pub resumed: bool,
}

#[derive(Debug, Error)]
pub enum ModulePayloadCacheError {
    #[error("module payload cache request is invalid")]
    InvalidRequest,
    #[error("module payload cache digest mismatch: expected {expected}, received {received}")]
    DigestMismatch { expected: String, received: String },
    #[error("module payload cache entry is not a regular file: {0}")]
    UnsafeCacheEntry(String),
    #[error("module payload cache preparation marker is invalid: {0}")]
    InvalidPreparationMarker(String),
    #[error(transparent)]
    Layout(#[from] InstanceLayoutError),
    #[error("module payload cache I/O failed at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("module payload cache serialization failed at {path}: {source}")]
    Serialization {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

/// Atomically materializes exact CAS bytes at the canonical digest-addressed
/// payload path. A valid pre-existing file is reused after a full rehash. A
/// corrupt regular cache file is replaced from the caller's verified bytes;
/// links and non-files fail closed and are never deleted by this helper.
pub fn materialize_module_payload(
    layout: &InstanceLayout,
    payload_digest: &str,
    bytes: &[u8],
) -> Result<ModulePayloadMaterializationReceipt, ModulePayloadCacheError> {
    let target = layout.module_payload_cache_entry(payload_digest)?;
    let expected_size =
        u64::try_from(bytes.len()).map_err(|_| ModulePayloadCacheError::InvalidRequest)?;
    verify_bytes(payload_digest, bytes)?;
    ensure_parent(layout, &target)?;

    match verify_regular_digest(&target, payload_digest) {
        Ok(()) => {
            return Ok(ModulePayloadMaterializationReceipt {
                payload_digest: payload_digest.to_string(),
                payload_path: target.display().to_string(),
                size_bytes: expected_size,
                resumed: true,
            });
        }
        Err(ModulePayloadCacheError::DigestMismatch { .. }) => {
            remove_corrupt_regular_file(&target)?;
        }
        Err(ModulePayloadCacheError::Io { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }

    write_verified_file(&target, payload_digest, bytes)?;
    Ok(ModulePayloadMaterializationReceipt {
        payload_digest: payload_digest.to_string(),
        payload_path: target.display().to_string(),
        size_bytes: expected_size,
        resumed: false,
    })
}

/// Records that one exact payload was prepared for one exact runtime
/// fingerprint. The payload cache entry is rehashed before the marker can be
/// created or reused, so a stale marker never turns a corrupt payload into a
/// readiness signal.
pub fn record_prepared_module(
    layout: &InstanceLayout,
    runtime_fingerprint: &str,
    payload_digest: &str,
) -> Result<PreparedModuleCacheReceipt, ModulePayloadCacheError> {
    let payload_path = layout.module_payload_cache_entry(payload_digest)?;
    verify_regular_digest(&payload_path, payload_digest)?;
    let preparation_path =
        layout.prepared_module_cache_entry(runtime_fingerprint, payload_digest)?;
    ensure_parent(layout, &preparation_path)?;

    let expected = PreparedModuleCacheReceipt {
        runtime_fingerprint: runtime_fingerprint.to_string(),
        payload_digest: payload_digest.to_string(),
        payload_path: payload_path.display().to_string(),
        preparation_path: preparation_path.display().to_string(),
        resumed: false,
    };
    match read_preparation_marker(&preparation_path) {
        Ok(Some(stored)) if same_preparation(&stored, &expected) => {
            return Ok(PreparedModuleCacheReceipt {
                resumed: true,
                ..stored
            });
        }
        Ok(Some(_)) | Err(ModulePayloadCacheError::InvalidPreparationMarker(_)) => {
            replace_regular_file(&preparation_path)?;
        }
        Ok(None) => {}
        Err(error) => return Err(error),
    }
    write_json_atomically(&preparation_path, &expected)?;
    Ok(expected)
}

fn same_preparation(
    actual: &PreparedModuleCacheReceipt,
    expected: &PreparedModuleCacheReceipt,
) -> bool {
    actual.runtime_fingerprint == expected.runtime_fingerprint
        && actual.payload_digest == expected.payload_digest
        && actual.payload_path == expected.payload_path
        && actual.preparation_path == expected.preparation_path
}

fn read_preparation_marker(
    path: &Path,
) -> Result<Option<PreparedModuleCacheReceipt>, ModulePayloadCacheError> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(path, source)),
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ModulePayloadCacheError::UnsafeCacheEntry(
            path.display().to_string(),
        ));
    }
    let bytes = fs::read(path).map_err(|source| io_error(path, source))?;
    serde_json::from_slice(&bytes)
        .map(Some)
        .map_err(|_| ModulePayloadCacheError::InvalidPreparationMarker(path.display().to_string()))
}

fn ensure_parent(layout: &InstanceLayout, target: &Path) -> Result<(), ModulePayloadCacheError> {
    let parent = target
        .parent()
        .ok_or(ModulePayloadCacheError::InvalidRequest)?;
    crate::layout::reject_managed_links(layout.root(), parent)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    crate::layout::reject_managed_links(layout.root(), parent)?;
    Ok(())
}

fn remove_corrupt_regular_file(path: &Path) -> Result<(), ModulePayloadCacheError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ModulePayloadCacheError::UnsafeCacheEntry(
            path.display().to_string(),
        ));
    }
    fs::remove_file(path).map_err(|source| io_error(path, source))
}

fn replace_regular_file(path: &Path) -> Result<(), ModulePayloadCacheError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_symlink() && metadata.is_file() => {
            fs::remove_file(path).map_err(|source| io_error(path, source))
        }
        Ok(_) => Err(ModulePayloadCacheError::UnsafeCacheEntry(
            path.display().to_string(),
        )),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_error(path, source)),
    }
}

fn write_verified_file(
    target: &Path,
    expected_digest: &str,
    bytes: &[u8],
) -> Result<(), ModulePayloadCacheError> {
    let parent = target
        .parent()
        .ok_or(ModulePayloadCacheError::InvalidRequest)?;
    let pending = pending_path(parent, target)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|source| io_error(&pending, source))?;
    let mut hasher = Sha256::new();
    for chunk in bytes.chunks(64 * 1024) {
        hasher.update(chunk);
        file.write_all(chunk)
            .map_err(|source| io_error(&pending, source))?;
    }
    file.flush().map_err(|source| io_error(&pending, source))?;
    file.sync_all()
        .map_err(|source| io_error(&pending, source))?;
    drop(file);
    let received = digest_from_hasher(hasher);
    if received != expected_digest {
        let _ = fs::remove_file(&pending);
        return Err(ModulePayloadCacheError::DigestMismatch {
            expected: expected_digest.to_string(),
            received,
        });
    }
    match fs::rename(&pending, target) {
        Ok(()) => Ok(()),
        Err(_source) if target.exists() => {
            let _ = fs::remove_file(&pending);
            verify_regular_digest(target, expected_digest)
        }
        Err(source) => {
            let _ = fs::remove_file(&pending);
            Err(io_error(target, source))
        }
    }
}

fn write_json_atomically<T: Serialize>(
    target: &Path,
    value: &T,
) -> Result<(), ModulePayloadCacheError> {
    let bytes =
        serde_json::to_vec(value).map_err(|source| ModulePayloadCacheError::Serialization {
            path: target.display().to_string(),
            source,
        })?;
    let parent = target
        .parent()
        .ok_or(ModulePayloadCacheError::InvalidRequest)?;
    let pending = pending_path(parent, target)?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&pending)
        .map_err(|source| io_error(&pending, source))?;
    file.write_all(&bytes)
        .map_err(|source| io_error(&pending, source))?;
    file.sync_all()
        .map_err(|source| io_error(&pending, source))?;
    drop(file);
    match fs::rename(&pending, target) {
        Ok(()) => Ok(()),
        Err(_source) if target.exists() => {
            let _ = fs::remove_file(&pending);
            Err(ModulePayloadCacheError::InvalidPreparationMarker(
                target.display().to_string(),
            ))
        }
        Err(source) => {
            let _ = fs::remove_file(&pending);
            Err(io_error(target, source))
        }
    }
}

fn pending_path(parent: &Path, target: &Path) -> Result<PathBuf, ModulePayloadCacheError> {
    let name = target
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or(ModulePayloadCacheError::InvalidRequest)?;
    Ok(parent.join(format!(".{name}.{}.pending", Uuid::new_v4())))
}

fn verify_bytes(expected_digest: &str, bytes: &[u8]) -> Result<(), ModulePayloadCacheError> {
    let received = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
    if received == expected_digest {
        Ok(())
    } else {
        Err(ModulePayloadCacheError::DigestMismatch {
            expected: expected_digest.to_string(),
            received,
        })
    }
}

fn verify_regular_digest(
    path: &Path,
    expected_digest: &str,
) -> Result<(), ModulePayloadCacheError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ModulePayloadCacheError::UnsafeCacheEntry(
            path.display().to_string(),
        ));
    }
    let mut file = File::open(path).map_err(|source| io_error(path, source))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|source| io_error(path, source))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
    }
    let received = digest_from_hasher(hasher);
    if received == expected_digest {
        Ok(())
    } else {
        Err(ModulePayloadCacheError::DigestMismatch {
            expected: expected_digest.to_string(),
            received,
        })
    }
}

fn digest_from_hasher(hasher: Sha256) -> String {
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

fn io_error(path: impl AsRef<Path>, source: std::io::Error) -> ModulePayloadCacheError {
    ModulePayloadCacheError::Io {
        path: path.as_ref().display().to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    use super::{ModulePayloadCacheError, materialize_module_payload, record_prepared_module};
    use crate::{InstanceLayout, InstancePlacement, prepare_instance_layout};

    fn layout() -> (InstanceLayout, std::path::PathBuf) {
        let parent = std::env::temp_dir().join(format!("rustok-module-cache-{}", Uuid::new_v4()));
        let layout = InstanceLayout::resolve(
            InstancePlacement::new(parent.join("instance").display().to_string()),
            &parent,
        )
        .expect("layout");
        prepare_instance_layout(&layout).expect("prepare layout");
        (layout, parent)
    }

    #[test]
    fn materializes_rehashes_and_records_preparation_for_one_exact_digest() {
        let (layout, parent) = layout();
        let bytes = b"node-local module payload";
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
        let runtime = format!("sha256:{}", "a".repeat(64));

        let first = materialize_module_payload(&layout, &digest, bytes).expect("first cache write");
        assert!(!first.resumed);
        assert!(
            materialize_module_payload(&layout, &digest, bytes)
                .expect("cache replay")
                .resumed
        );
        let prepared = record_prepared_module(&layout, &runtime, &digest).expect("prepare marker");
        assert!(!prepared.resumed);
        assert!(
            record_prepared_module(&layout, &runtime, &digest)
                .expect("preparation replay")
                .resumed
        );
        std::fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn replaces_a_corrupt_regular_payload_cache_entry_from_verified_bytes() {
        let (layout, parent) = layout();
        let bytes = b"node-local module payload";
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(bytes)));
        materialize_module_payload(&layout, &digest, bytes).expect("cache write");
        let path = layout
            .module_payload_cache_entry(&digest)
            .expect("cache path");
        std::fs::write(&path, b"corrupt").expect("corrupt cache");

        let repaired = materialize_module_payload(&layout, &digest, bytes).expect("repair cache");
        assert!(!repaired.resumed);
        assert_eq!(std::fs::read(path).expect("read repaired cache"), bytes);
        std::fs::remove_dir_all(parent).expect("cleanup");
    }

    #[test]
    fn rejects_mismatched_bytes_without_creating_a_cache_file() {
        let (layout, parent) = layout();
        let digest = format!("sha256:{}", "b".repeat(64));
        let error =
            materialize_module_payload(&layout, &digest, b"wrong").expect_err("digest mismatch");
        assert!(matches!(
            error,
            ModulePayloadCacheError::DigestMismatch { .. }
        ));
        assert!(
            !layout
                .module_payload_cache_entry(&digest)
                .expect("cache path")
                .exists()
        );
        std::fs::remove_dir_all(parent).expect("cleanup");
    }
}
