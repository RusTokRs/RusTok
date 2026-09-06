//! Node-local, digest-verified static-role materialization.
//!
//! This is deliberately a narrow operations primitive. It accepts only an
//! already pre-staged cache entry selected by the control plane, writes no
//! database state, starts no arbitrary process, and never resolves tags or
//! contacts a registry. The deployment agent composes this primitive with its
//! authenticated owner assignment and process supervisor.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{InstanceLayout, InstanceLayoutError};

const ARTIFACT_FILE: &str = "artifact";
const RECEIPT_FILE: &str = "materialization.json";

/// Exact local work accepted from an owner-issued role assignment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleMaterializationRequest {
    pub operation_id: Uuid,
    pub role: String,
    pub bundle_digest: String,
    pub artifact_digest: String,
}

/// Non-authoritative local receipt. It permits exact restart replay, while the
/// control plane keeps desired/observed rollout identity and activation state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleMaterializationReceipt {
    pub operation_id: Uuid,
    pub role: String,
    pub bundle_digest: String,
    pub artifact_digest: String,
    pub materialization_path: String,
    pub resumed: bool,
}

#[derive(Debug, Error)]
pub enum RoleMaterializationError {
    #[error("role materialization request is invalid")]
    InvalidRequest,
    #[error("required pre-staged artifact is missing: {0}")]
    MissingStagedArtifact(String),
    #[error("pre-staged artifact is not a regular file: {0}")]
    UnsafeStagedArtifact(String),
    #[error("artifact digest mismatch: expected {expected}, received {received}")]
    DigestMismatch { expected: String, received: String },
    #[error("existing materialization does not match the exact assignment: {0}")]
    ExistingMaterializationMismatch(String),
    #[error(transparent)]
    Layout(#[from] InstanceLayoutError),
    #[error("role materialization I/O failed at {path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },
    #[error("role materialization receipt is invalid at {path}: {source}")]
    Receipt {
        path: String,
        #[source]
        source: serde_json::Error,
    },
}

/// Materializes exactly one previously pre-staged role artifact beneath the
/// portable instance root and records an exact restart journal receipt.
pub fn materialize_role(
    layout: &InstanceLayout,
    request: RoleMaterializationRequest,
) -> Result<RoleMaterializationReceipt, RoleMaterializationError> {
    validate_request(&request)?;
    let staged = layout.deployment_cache_entry(&request.artifact_digest)?;
    verify_regular_digest(&staged, &request.artifact_digest)?;

    let role_path = layout.platform_role(&request.bundle_digest, &request.role)?;
    let receipt = RoleMaterializationReceipt {
        operation_id: request.operation_id,
        role: request.role.clone(),
        bundle_digest: request.bundle_digest.clone(),
        artifact_digest: request.artifact_digest.clone(),
        materialization_path: role_path.display().to_string(),
        resumed: false,
    };

    let receipt = if role_path.exists() {
        verify_existing_materialization(&role_path, &receipt)?
    } else {
        create_materialization(&role_path, &staged, &receipt)?
    };
    write_journal(layout, &receipt)?;
    Ok(receipt)
}

fn validate_request(request: &RoleMaterializationRequest) -> Result<(), RoleMaterializationError> {
    if request.operation_id.is_nil()
        || !valid_digest(&request.bundle_digest)
        || !valid_digest(&request.artifact_digest)
        || !valid_role(&request.role)
    {
        return Err(RoleMaterializationError::InvalidRequest);
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

fn valid_role(role: &str) -> bool {
    !role.is_empty()
        && role.len() <= 128
        && role.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_')
        })
}

fn create_materialization(
    target: &Path,
    staged: &Path,
    receipt: &RoleMaterializationReceipt,
) -> Result<RoleMaterializationReceipt, RoleMaterializationError> {
    let parent = target
        .parent()
        .ok_or(RoleMaterializationError::InvalidRequest)?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let pending = parent.join(format!(
        ".{}.{}.pending",
        receipt.role, receipt.operation_id
    ));
    let resumed_pending = match fs::create_dir(&pending) {
        Ok(()) => false,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => true,
        Err(error) => return Err(io_error(&pending, error)),
    };

    complete_pending_materialization(&pending, staged, receipt)?;
    match fs::rename(&pending, target) {
        Ok(()) => Ok(RoleMaterializationReceipt {
            resumed: resumed_pending,
            ..receipt.clone()
        }),
        Err(_error) if target.exists() => verify_existing_materialization(target, receipt),
        Err(error) => Err(io_error(target, error)),
    }
}

/// Completes only the exact pending directory created for this assignment.
/// A process death leaves it intact: a subsequent lease replay either verifies
/// and finishes it or fails closed on unexpected contents. No partially
/// materialized directory is deleted implicitly.
fn complete_pending_materialization(
    pending: &Path,
    staged: &Path,
    receipt: &RoleMaterializationReceipt,
) -> Result<(), RoleMaterializationError> {
    ensure_materialization_directory(pending)?;
    let artifact = pending.join(ARTIFACT_FILE);
    let receipt_path = pending.join(RECEIPT_FILE);

    if receipt_path.exists() {
        ensure_exact_materialization_entries(pending)?;
        verify_existing_materialization(pending, receipt)?;
        return Ok(());
    }
    if artifact.exists() {
        ensure_pending_artifact_only(pending)?;
        verify_regular_digest(&artifact, &receipt.artifact_digest)?;
    } else {
        ensure_empty_directory(pending)?;
        copy_and_verify(staged, &artifact, &receipt.artifact_digest)?;
    }
    write_new_json(&receipt_path, receipt)?;
    ensure_exact_materialization_entries(pending)?;
    verify_existing_materialization(pending, receipt)?;
    Ok(())
}

fn verify_existing_materialization(
    target: &Path,
    expected: &RoleMaterializationReceipt,
) -> Result<RoleMaterializationReceipt, RoleMaterializationError> {
    ensure_materialization_directory(target)?;
    ensure_exact_materialization_entries(target)?;
    let receipt_path = target.join(RECEIPT_FILE);
    let bytes = fs::read(&receipt_path).map_err(|source| io_error(&receipt_path, source))?;
    let stored: RoleMaterializationReceipt =
        serde_json::from_slice(&bytes).map_err(|source| RoleMaterializationError::Receipt {
            path: receipt_path.display().to_string(),
            source,
        })?;
    if stored.operation_id != expected.operation_id
        || stored.role != expected.role
        || stored.bundle_digest != expected.bundle_digest
        || stored.artifact_digest != expected.artifact_digest
        || stored.materialization_path != expected.materialization_path
    {
        return Err(RoleMaterializationError::ExistingMaterializationMismatch(
            target.display().to_string(),
        ));
    }
    verify_regular_digest(&target.join(ARTIFACT_FILE), &expected.artifact_digest)?;
    Ok(RoleMaterializationReceipt {
        resumed: true,
        ..stored
    })
}

fn ensure_materialization_directory(path: &Path) -> Result<(), RoleMaterializationError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| io_error(path, source))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(RoleMaterializationError::ExistingMaterializationMismatch(
            path.display().to_string(),
        ));
    }
    Ok(())
}

fn ensure_empty_directory(path: &Path) -> Result<(), RoleMaterializationError> {
    let mut entries = fs::read_dir(path).map_err(|source| io_error(path, source))?;
    if entries
        .next()
        .transpose()
        .map_err(|source| io_error(path, source))?
        .is_some()
    {
        return Err(RoleMaterializationError::ExistingMaterializationMismatch(
            path.display().to_string(),
        ));
    }
    Ok(())
}

fn ensure_pending_artifact_only(path: &Path) -> Result<(), RoleMaterializationError> {
    ensure_directory_entries(path, &[ARTIFACT_FILE])
}

fn ensure_exact_materialization_entries(path: &Path) -> Result<(), RoleMaterializationError> {
    ensure_directory_entries(path, &[ARTIFACT_FILE, RECEIPT_FILE])
}

fn ensure_directory_entries(path: &Path, allowed: &[&str]) -> Result<(), RoleMaterializationError> {
    let entries = fs::read_dir(path).map_err(|source| io_error(path, source))?;
    let mut count = 0_usize;
    for entry in entries {
        let entry = entry.map_err(|source| io_error(path, source))?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            return Err(RoleMaterializationError::ExistingMaterializationMismatch(
                path.display().to_string(),
            ));
        };
        let metadata =
            fs::symlink_metadata(entry.path()).map_err(|source| io_error(entry.path(), source))?;
        if !allowed.contains(&name) || metadata.file_type().is_symlink() || !metadata.is_file() {
            return Err(RoleMaterializationError::ExistingMaterializationMismatch(
                path.display().to_string(),
            ));
        }
        count += 1;
    }
    if count != allowed.len() {
        return Err(RoleMaterializationError::ExistingMaterializationMismatch(
            path.display().to_string(),
        ));
    }
    Ok(())
}

fn copy_and_verify(
    source: &Path,
    target: &Path,
    expected_digest: &str,
) -> Result<(), RoleMaterializationError> {
    let mut input = File::open(source).map_err(|source_error| io_error(source, source_error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(target)
        .map_err(|source_error| io_error(target, source_error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = input
            .read(&mut buffer)
            .map_err(|source_error| io_error(source, source_error))?;
        if count == 0 {
            break;
        }
        hasher.update(&buffer[..count]);
        output
            .write_all(&buffer[..count])
            .map_err(|source_error| io_error(target, source_error))?;
    }
    output
        .flush()
        .map_err(|source_error| io_error(target, source_error))?;
    output
        .sync_all()
        .map_err(|source_error| io_error(target, source_error))?;
    let received = format!("sha256:{}", hex::encode(hasher.finalize()));
    if received != expected_digest {
        return Err(RoleMaterializationError::DigestMismatch {
            expected: expected_digest.to_string(),
            received,
        });
    }
    Ok(())
}

fn verify_regular_digest(
    path: &Path,
    expected_digest: &str,
) -> Result<(), RoleMaterializationError> {
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            RoleMaterializationError::MissingStagedArtifact(path.display().to_string())
        } else {
            io_error(path, error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(RoleMaterializationError::UnsafeStagedArtifact(
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
    let received = format!("sha256:{}", hex::encode(hasher.finalize()));
    if received != expected_digest {
        return Err(RoleMaterializationError::DigestMismatch {
            expected: expected_digest.to_string(),
            received,
        });
    }
    Ok(())
}

fn write_journal(
    layout: &InstanceLayout,
    receipt: &RoleMaterializationReceipt,
) -> Result<(), RoleMaterializationError> {
    let journal = layout.deployment_operation_journal(&receipt.operation_id.to_string())?;
    fs::create_dir_all(&journal).map_err(|source| io_error(&journal, source))?;
    let path = journal.join(format!("{}.json", receipt.role));
    if path.exists() {
        let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
        let stored: RoleMaterializationReceipt =
            serde_json::from_slice(&bytes).map_err(|source| RoleMaterializationError::Receipt {
                path: path.display().to_string(),
                source,
            })?;
        if stored.operation_id == receipt.operation_id
            && stored.role == receipt.role
            && stored.bundle_digest == receipt.bundle_digest
            && stored.artifact_digest == receipt.artifact_digest
            && stored.materialization_path == receipt.materialization_path
        {
            return Ok(());
        }
        return Err(RoleMaterializationError::ExistingMaterializationMismatch(
            path.display().to_string(),
        ));
    }
    write_new_json(&path, receipt)
}

fn write_new_json<T: Serialize>(path: &Path, value: &T) -> Result<(), RoleMaterializationError> {
    let bytes = serde_json::to_vec_pretty(value).expect("deployment receipt must serialize");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| io_error(path, source))?;
    file.write_all(&bytes)
        .map_err(|source| io_error(path, source))?;
    file.sync_all().map_err(|source| io_error(path, source))
}

fn io_error(path: impl AsRef<Path>, source: std::io::Error) -> RoleMaterializationError {
    RoleMaterializationError::Io {
        path: path.as_ref().display().to_string(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::{RoleMaterializationError, RoleMaterializationRequest, materialize_role};
    use crate::{InstanceLayout, InstancePlacement, prepare_instance_layout};
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    #[test]
    fn materializes_only_the_verified_pre_staged_digest_and_replays() {
        let parent = std::env::temp_dir().join(format!("rustok-deployment-{}", Uuid::new_v4()));
        let layout = InstanceLayout::resolve(
            InstancePlacement::new(parent.join("instance").display().to_string()),
            &parent,
        )
        .unwrap();
        prepare_instance_layout(&layout).unwrap();
        let artifact = b"candidate role bytes";
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(artifact)));
        let staged = layout.deployment_cache_entry(&digest).unwrap();
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(&staged, artifact).unwrap();
        let request = RoleMaterializationRequest {
            operation_id: Uuid::new_v4(),
            role: "api".to_string(),
            bundle_digest: format!("sha256:{}", "a".repeat(64)),
            artifact_digest: digest,
        };
        let first = materialize_role(&layout, request.clone()).unwrap();
        assert!(!first.resumed);
        assert_eq!(
            std::fs::read(
                layout
                    .platform_role(&request.bundle_digest, "api")
                    .unwrap()
                    .join("artifact")
            )
            .unwrap(),
            artifact,
        );
        assert!(materialize_role(&layout, request).unwrap().resumed);
        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn rejects_a_staged_file_with_a_different_digest() {
        let parent = std::env::temp_dir().join(format!("rustok-deployment-{}", Uuid::new_v4()));
        let layout = InstanceLayout::resolve(
            InstancePlacement::new(parent.join("instance").display().to_string()),
            &parent,
        )
        .unwrap();
        prepare_instance_layout(&layout).unwrap();
        let expected = format!("sha256:{}", "b".repeat(64));
        let staged = layout.deployment_cache_entry(&expected).unwrap();
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(&staged, "wrong bytes").unwrap();
        let error = materialize_role(
            &layout,
            RoleMaterializationRequest {
                operation_id: Uuid::new_v4(),
                role: "worker".to_string(),
                bundle_digest: format!("sha256:{}", "a".repeat(64)),
                artifact_digest: expected,
            },
        )
        .unwrap_err();
        assert!(matches!(
            error,
            RoleMaterializationError::DigestMismatch { .. }
        ));
        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn resumes_a_verified_pending_copy_without_deleting_it() {
        let parent = std::env::temp_dir().join(format!("rustok-deployment-{}", Uuid::new_v4()));
        let layout = InstanceLayout::resolve(
            InstancePlacement::new(parent.join("instance").display().to_string()),
            &parent,
        )
        .unwrap();
        prepare_instance_layout(&layout).unwrap();
        let artifact = b"resumable candidate role bytes";
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(artifact)));
        let staged = layout.deployment_cache_entry(&digest).unwrap();
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(&staged, artifact).unwrap();
        let request = RoleMaterializationRequest {
            operation_id: Uuid::new_v4(),
            role: "worker".to_string(),
            bundle_digest: format!("sha256:{}", "a".repeat(64)),
            artifact_digest: digest,
        };
        let target = layout
            .platform_role(&request.bundle_digest, &request.role)
            .unwrap();
        let pending = target.parent().unwrap().join(format!(
            ".{}.{}.pending",
            request.role, request.operation_id
        ));
        std::fs::create_dir_all(&pending).unwrap();
        std::fs::write(pending.join("artifact"), artifact).unwrap();

        let receipt = materialize_role(&layout, request).unwrap();
        assert!(receipt.resumed);
        assert!(target.exists());
        assert!(!pending.exists());
        std::fs::remove_dir_all(parent).unwrap();
    }

    #[test]
    fn preserves_an_unexpected_pending_directory_for_operator_inspection() {
        let parent = std::env::temp_dir().join(format!("rustok-deployment-{}", Uuid::new_v4()));
        let layout = InstanceLayout::resolve(
            InstancePlacement::new(parent.join("instance").display().to_string()),
            &parent,
        )
        .unwrap();
        prepare_instance_layout(&layout).unwrap();
        let artifact = b"candidate role bytes";
        let digest = format!("sha256:{}", hex::encode(Sha256::digest(artifact)));
        let staged = layout.deployment_cache_entry(&digest).unwrap();
        std::fs::create_dir_all(staged.parent().unwrap()).unwrap();
        std::fs::write(&staged, artifact).unwrap();
        let request = RoleMaterializationRequest {
            operation_id: Uuid::new_v4(),
            role: "api".to_string(),
            bundle_digest: format!("sha256:{}", "a".repeat(64)),
            artifact_digest: digest,
        };
        let target = layout
            .platform_role(&request.bundle_digest, &request.role)
            .unwrap();
        let pending = target.parent().unwrap().join(format!(
            ".{}.{}.pending",
            request.role, request.operation_id
        ));
        std::fs::create_dir_all(&pending).unwrap();
        let unexpected = pending.join("operator-note");
        std::fs::write(&unexpected, "do not delete").unwrap();

        let error = materialize_role(&layout, request).unwrap_err();
        assert!(matches!(
            error,
            RoleMaterializationError::ExistingMaterializationMismatch(_)
        ));
        assert!(unexpected.exists());
        std::fs::remove_dir_all(parent).unwrap();
    }
}
