//! Reviewed Rust Component source for an Alloy evolution candidate.
//!
//! This is deliberately a data-only source snapshot. It carries no filesystem
//! path, permission bit, timestamp, archive, build result, or execution claim.
//! A host materializes it through `SourceTreeMaterializer` only after the exact
//! snapshot has been persisted and reviewed.

use std::collections::BTreeSet;

use chrono::{DateTime, Utc};
use rustok_build_source::{ArchiveLimits, SourceTreeFile, SourceTreeMaterializer};
use rustok_modules::{
    ArtifactReleaseRef, MODULE_ARTIFACT_SOURCE_MANIFEST_FILE, ModuleArtifactSourceManifest,
    ModuleAuthoringBuildSubmission, ModuleCommandContext,
};
use rustok_sandbox::LocalSandboxScenario;
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::{ReviewError, ReviewStatus, ScriptId};

pub const MAX_RUST_COMPONENT_SOURCE_FILES: usize = 2_048;
pub const MAX_RUST_COMPONENT_SOURCE_FILE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_RUST_COMPONENT_SOURCE_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_RUST_COMPONENT_SOURCE_PATH_BYTES: usize = 255;
pub const MAX_RUST_COMPONENT_CANDIDATE_ACTOR_ID_LENGTH: usize = 255;

const REQUIRED_PATHS: &[&str] = &[
    "Cargo.toml",
    "Cargo.lock",
    "src/lib.rs",
    "rust-toolchain.toml",
    "module-build-policy.toml",
    MODULE_ARTIFACT_SOURCE_MANIFEST_FILE,
    "tests/sandbox-scenario.json",
];

/// One immutable reviewed Rust Component source snapshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentWorkspace {
    pub files: Vec<RustComponentSourceFile>,
}

/// UTF-8 source file supplied for a reviewed Component rewrite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentSourceFile {
    pub path: String,
    pub contents: String,
}

/// An authenticated request to persist one immutable Rust Component rewrite
/// candidate for a reviewed Rhai draft revision. The request is data-only:
/// hosts, transports, and persistence never receive a caller filesystem path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentCandidateCommand {
    pub script_id: ScriptId,
    pub expected_revision: u32,
    pub workspace: RustComponentWorkspace,
    pub actor_id: String,
    pub idempotency_key: Uuid,
}

impl RustComponentCandidateCommand {
    pub fn validate(&self) -> Result<(), RustComponentCandidateError> {
        if self.script_id.is_nil()
            || self.expected_revision == 0
            || self.idempotency_key.is_nil()
            || !is_bounded_value(&self.actor_id, MAX_RUST_COMPONENT_CANDIDATE_ACTOR_ID_LENGTH)
        {
            return Err(RustComponentCandidateError::InvalidCommand);
        }
        self.workspace
            .validate()
            .map_err(RustComponentCandidateError::Workspace)?;
        Ok(())
    }

    pub fn request_digest(&self) -> Result<String, RustComponentCandidateError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| RustComponentCandidateError::Serialization(error.to_string()))?;
        Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }
}

/// Durable immutable rewrite input, pinned to the approved Rhai source and
/// its exact published parent release. A later draft revision or release can
/// never reinterpret this candidate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentCandidate {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub script_id: ScriptId,
    pub parent_revision: u32,
    pub parent_source_digest: String,
    pub parent_release: ArtifactReleaseRef,
    pub workspace: RustComponentWorkspace,
    pub source_digest: String,
    pub scenario_digest: String,
    pub actor_id: String,
    pub idempotency_key: Uuid,
    pub request_digest: String,
    pub created_at: DateTime<Utc>,
}

/// Authenticated review transition for one immutable Rust Component candidate.
/// Its decision never applies to another candidate, even when both originated
/// from the same Rhai revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentCandidateReviewCommand {
    pub candidate_id: Uuid,
    pub status: ReviewStatus,
    pub policy_revision: String,
    pub actor_id: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
}

impl RustComponentCandidateReviewCommand {
    pub fn validate(&self) -> Result<(), ReviewError> {
        if self.candidate_id.is_nil()
            || self.idempotency_key.is_nil()
            || !is_bounded_value(
                &self.policy_revision,
                super::MAX_REVIEW_POLICY_REVISION_LENGTH,
            )
            || !is_bounded_value(&self.actor_id, super::MAX_REVIEW_ACTOR_ID_LENGTH)
            || self.reason.as_ref().is_some_and(|reason| {
                reason.trim() != reason
                    || reason.is_empty()
                    || reason.len() > super::MAX_REVIEW_REASON_LENGTH
                    || reason.chars().any(char::is_control)
            })
        {
            return Err(ReviewError::InvalidCommand);
        }
        Ok(())
    }

    pub fn request_digest(&self) -> Result<String, ReviewError> {
        self.validate()?;
        let bytes =
            serde_json::to_vec(self).map_err(|error| ReviewError::Serialize(error.to_string()))?;
        Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }
}

/// Immutable authorized decision over one exact candidate source and scenario.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentCandidateReview {
    pub id: Uuid,
    pub candidate_id: Uuid,
    pub tenant_id: Uuid,
    pub source_digest: String,
    pub scenario_digest: String,
    pub status: ReviewStatus,
    pub policy_revision: String,
    pub actor_id: String,
    pub reason: Option<String>,
    pub idempotency_key: Uuid,
    pub request_digest: String,
    pub created_at: DateTime<Utc>,
}

/// Host-composed dispatch input for an approved candidate. It is not a remote
/// source-upload command: the host derives archive digest, module identity,
/// and parent release from the durable candidate before it calls build control.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentCandidateBuildCommand {
    pub candidate_id: Uuid,
    pub context: ModuleCommandContext,
    pub project_id: String,
    pub rust_toolchain: String,
    pub sdk_version: String,
    pub template_version: String,
    pub dependency_lock_digest: String,
}

impl RustComponentCandidateBuildCommand {
    pub fn validate(&self) -> Result<(), RustComponentCandidateBuildError> {
        if self.candidate_id.is_nil()
            || self.project_id.trim().is_empty()
            || self.project_id.len() > 256
            || self.project_id.contains(char::is_control)
            || Version::parse(&self.rust_toolchain).is_err()
            || Version::parse(&self.sdk_version).is_err()
            || Version::parse(&self.template_version).is_err()
            || !valid_digest(&self.dependency_lock_digest)
        {
            return Err(RustComponentCandidateBuildError::InvalidCommand);
        }
        self.context
            .validate()
            .map_err(|_| RustComponentCandidateBuildError::InvalidCommand)
    }

    pub fn request_digest(&self) -> Result<String, RustComponentCandidateBuildError> {
        self.validate()?;
        let bytes = serde_json::to_vec(self)
            .map_err(|error| RustComponentCandidateBuildError::Serialization(error.to_string()))?;
        Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
    }
}

/// Durable linkage from one approved candidate to exactly one owner build
/// request for an idempotency key. The archive digest is distinct from the
/// candidate's canonical data-only source digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RustComponentCandidateBuild {
    pub id: Uuid,
    pub candidate_id: Uuid,
    pub tenant_id: Uuid,
    pub candidate_source_digest: String,
    pub scenario_digest: String,
    pub archive_source_digest: String,
    pub build_request_id: Uuid,
    pub source_reference: String,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
    pub request_digest: String,
    pub created_at: DateTime<Utc>,
}

impl RustComponentCandidateBuild {
    pub fn from_submission(
        candidate: &RustComponentCandidate,
        command: &RustComponentCandidateBuildCommand,
        submission: &ModuleAuthoringBuildSubmission,
    ) -> Result<Self, RustComponentCandidateBuildError> {
        command.validate()?;
        if command.candidate_id != candidate.id
            || command.context.tenant_id != Some(candidate.tenant_id)
            || command.context.idempotency_key.is_nil()
            || submission.request_id.is_nil()
            || !valid_digest(&submission.source_digest)
            || submission.source_reference != format!("cas://{}", submission.source_digest)
        {
            return Err(RustComponentCandidateBuildError::InvalidSubmission);
        }
        let build = Self {
            id: Uuid::new_v4(),
            candidate_id: candidate.id,
            tenant_id: candidate.tenant_id,
            candidate_source_digest: candidate.source_digest.clone(),
            scenario_digest: candidate.scenario_digest.clone(),
            archive_source_digest: submission.source_digest.clone(),
            build_request_id: submission.request_id,
            source_reference: submission.source_reference.clone(),
            actor_id: command.context.actor_id,
            idempotency_key: command.context.idempotency_key,
            request_digest: command.request_digest()?,
            created_at: Utc::now(),
        };
        build.validate_against(candidate)?;
        Ok(build)
    }

    /// Checks the durable receipt against the immutable candidate before it
    /// enters any storage implementation.
    pub fn validate_against(
        &self,
        candidate: &RustComponentCandidate,
    ) -> Result<(), RustComponentCandidateBuildError> {
        if self.id.is_nil()
            || self.candidate_id != candidate.id
            || self.tenant_id != candidate.tenant_id
            || self.candidate_source_digest != candidate.source_digest
            || self.scenario_digest != candidate.scenario_digest
            || !valid_digest(&self.candidate_source_digest)
            || !valid_digest(&self.scenario_digest)
            || !valid_digest(&self.archive_source_digest)
            || self.build_request_id.is_nil()
            || self.actor_id.is_nil()
            || self.idempotency_key.is_nil()
            || !valid_digest(&self.request_digest)
            || self.source_reference != format!("cas://{}", self.archive_source_digest)
        {
            return Err(RustComponentCandidateBuildError::InvalidSubmission);
        }
        Ok(())
    }
}

impl RustComponentWorkspace {
    /// Validates source shape without materializing it or evaluating author
    /// code. The same shared materializer validates path safety before any
    /// eventual host filesystem write.
    pub fn validate(&self) -> Result<(), RustComponentWorkspaceError> {
        if self.files.is_empty() || self.files.len() > MAX_RUST_COMPONENT_SOURCE_FILES {
            return Err(RustComponentWorkspaceError::InvalidFileCount);
        }
        let mut source_bytes = 0_usize;
        let mut paths = BTreeSet::new();
        let files = self
            .files
            .iter()
            .map(|file| {
                if file.path.is_empty()
                    || file.path.len() > MAX_RUST_COMPONENT_SOURCE_PATH_BYTES
                    || file.contents.len() > MAX_RUST_COMPONENT_SOURCE_FILE_BYTES
                    || !paths.insert(file.path.as_str())
                {
                    return Err(RustComponentWorkspaceError::InvalidFile);
                }
                source_bytes = source_bytes
                    .checked_add(file.contents.len())
                    .ok_or(RustComponentWorkspaceError::ResourceLimit)?;
                if source_bytes > MAX_RUST_COMPONENT_SOURCE_BYTES {
                    return Err(RustComponentWorkspaceError::ResourceLimit);
                }
                Ok(SourceTreeFile {
                    path: file.path.clone(),
                    contents: file.contents.as_bytes().to_vec(),
                })
            })
            .collect::<Result<Vec<_>, RustComponentWorkspaceError>>()?;
        let limits = ArchiveLimits::new(
            u64::try_from(MAX_RUST_COMPONENT_SOURCE_BYTES)
                .map_err(|_| RustComponentWorkspaceError::ResourceLimit)?,
            u64::try_from(MAX_RUST_COMPONENT_SOURCE_BYTES)
                .map_err(|_| RustComponentWorkspaceError::ResourceLimit)?,
            u32::try_from(MAX_RUST_COMPONENT_SOURCE_FILES)
                .map_err(|_| RustComponentWorkspaceError::ResourceLimit)?,
        )
        .map_err(|_| RustComponentWorkspaceError::ResourceLimit)?;
        SourceTreeMaterializer::new(limits)
            .validate(&files)
            .map_err(|_| RustComponentWorkspaceError::InvalidFile)?;
        for path in REQUIRED_PATHS {
            if !paths.contains(path) {
                return Err(RustComponentWorkspaceError::MissingRequiredFile(
                    (*path).to_string(),
                ));
            }
        }
        self.source_manifest()?;
        self.scenario()?;
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, RustComponentWorkspaceError> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical
            .files
            .sort_by(|left, right| left.path.cmp(&right.path));
        serde_json::to_vec(&canonical)
            .map_err(|error| RustComponentWorkspaceError::Serialization(error.to_string()))
    }

    pub fn source_digest(&self) -> Result<String, RustComponentWorkspaceError> {
        Ok(format!(
            "sha256:{}",
            hex::encode(Sha256::digest(self.canonical_bytes()?))
        ))
    }

    pub fn source_manifest(
        &self,
    ) -> Result<ModuleArtifactSourceManifest, RustComponentWorkspaceError> {
        let bytes = self
            .file(MODULE_ARTIFACT_SOURCE_MANIFEST_FILE)
            .ok_or_else(|| {
                RustComponentWorkspaceError::MissingRequiredFile(
                    MODULE_ARTIFACT_SOURCE_MANIFEST_FILE.to_string(),
                )
            })?
            .as_bytes();
        ModuleArtifactSourceManifest::parse(bytes)
            .map_err(|_| RustComponentWorkspaceError::InvalidSourceManifest)
    }

    pub fn scenario(&self) -> Result<LocalSandboxScenario, RustComponentWorkspaceError> {
        let bytes = self
            .file("tests/sandbox-scenario.json")
            .ok_or_else(|| {
                RustComponentWorkspaceError::MissingRequiredFile(
                    "tests/sandbox-scenario.json".to_string(),
                )
            })?
            .as_bytes();
        LocalSandboxScenario::parse(bytes).map_err(|_| RustComponentWorkspaceError::InvalidScenario)
    }

    pub fn source_files(&self) -> Result<Vec<SourceTreeFile>, RustComponentWorkspaceError> {
        self.validate()?;
        Ok(self
            .files
            .iter()
            .map(|file| SourceTreeFile {
                path: file.path.clone(),
                contents: file.contents.as_bytes().to_vec(),
            })
            .collect())
    }

    fn file(&self, path: &str) -> Option<&str> {
        self.files
            .iter()
            .find(|file| file.path == path)
            .map(|file| file.contents.as_str())
    }
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RustComponentWorkspaceError {
    #[error("Rust Component source has an invalid file count")]
    InvalidFileCount,
    #[error("Rust Component source contains an invalid file")]
    InvalidFile,
    #[error("Rust Component source exceeds its bounded resource policy")]
    ResourceLimit,
    #[error("Rust Component source is missing required file {0}")]
    MissingRequiredFile(String),
    #[error("Rust Component source manifest is invalid")]
    InvalidSourceManifest,
    #[error("Rust Component source scenario is invalid")]
    InvalidScenario,
    #[error("Rust Component source serialization failed: {0}")]
    Serialization(String),
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RustComponentCandidateError {
    #[error("Rust Component candidate command is invalid")]
    InvalidCommand,
    #[error("Rust Component candidate workspace is invalid: {0}")]
    Workspace(#[source] RustComponentWorkspaceError),
    #[error("Rust Component candidate serialization failed: {0}")]
    Serialization(String),
    #[error("Rust Component candidate idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("Rust Component candidate requires an approved current Rhai revision")]
    ParentReviewNotApproved,
    #[error("Rust Component candidate requires an exact published Rhai parent release")]
    ParentReleaseMissing,
    #[error("Rust Component candidate source manifest does not continue its Rhai parent release")]
    ParentReleaseMismatch,
}

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum RustComponentCandidateBuildError {
    #[error("Rust Component candidate build command is invalid")]
    InvalidCommand,
    #[error("Rust Component candidate build submission is invalid")]
    InvalidSubmission,
    #[error("Rust Component candidate build serialization failed: {0}")]
    Serialization(String),
    #[error("Rust Component candidate build idempotency key was reused for a different command")]
    IdempotencyConflict,
    #[error("Rust Component candidate must be approved before it can be dispatched")]
    CandidateNotApproved,
}

pub fn validate_candidate_parent_release(
    workspace: &RustComponentWorkspace,
    parent_release: &ArtifactReleaseRef,
) -> Result<(), RustComponentCandidateError> {
    let manifest = workspace
        .source_manifest()
        .map_err(RustComponentCandidateError::Workspace)?;
    let candidate_version = Version::parse(manifest.version())
        .map_err(|_| RustComponentCandidateError::ParentReleaseMismatch)?;
    let parent_version = Version::parse(&parent_release.version)
        .map_err(|_| RustComponentCandidateError::ParentReleaseMismatch)?;
    if manifest.slug() != parent_release.slug || candidate_version <= parent_version {
        return Err(RustComponentCandidateError::ParentReleaseMismatch);
    }
    Ok(())
}

fn is_bounded_value(value: &str, limit: usize) -> bool {
    value.trim() == value
        && !value.is_empty()
        && value.len() <= limit
        && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value.as_bytes()[7..]
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn workspace() -> RustComponentWorkspace {
        let rendered =
            rustok_module_template::render(&rustok_module_template::ModuleTemplateInput {
                slug: "sample_module".to_string(),
                version: "0.1.0".to_string(),
                display_name: "Sample Module".to_string(),
            })
            .expect("rendered module template");
        let mut files = rendered
            .files()
            .iter()
            .map(|file| RustComponentSourceFile {
                path: file.path.to_string(),
                contents: String::from_utf8(file.contents.clone()).expect("UTF-8 template file"),
            })
            .collect::<Vec<_>>();
        files.push(RustComponentSourceFile {
            path: "Cargo.lock".to_string(),
            contents: "# This file is automatically generated by Cargo.\nversion = 4\n".to_string(),
        });
        RustComponentWorkspace { files }
    }

    #[test]
    fn source_digest_is_independent_of_file_order_and_binds_the_scenario() {
        let first = workspace();
        let mut reordered = first.clone();
        reordered.files.reverse();

        assert!(first.validate().is_ok());
        assert_eq!(first.source_digest(), reordered.source_digest());
        assert_eq!(
            first
                .scenario()
                .expect("candidate scenario")
                .canonical_digest()
                .expect("scenario digest"),
            reordered
                .scenario()
                .expect("reordered candidate scenario")
                .canonical_digest()
                .expect("reordered scenario digest")
        );
    }

    #[test]
    fn source_rejects_missing_required_or_unsafe_files() {
        let mut missing = workspace();
        missing
            .files
            .retain(|file| file.path != "module-build-policy.toml");
        assert!(matches!(
            missing.validate(),
            Err(RustComponentWorkspaceError::MissingRequiredFile(_))
        ));

        let mut unsafe_file = workspace();
        unsafe_file.files.push(RustComponentSourceFile {
            path: ".cargo/config.toml".to_string(),
            contents: "[net]\noffline = false\n".to_string(),
        });
        assert_eq!(
            unsafe_file.validate(),
            Err(RustComponentWorkspaceError::InvalidFile)
        );
    }

    #[test]
    fn candidate_command_binds_the_complete_reviewed_workspace() {
        let command = RustComponentCandidateCommand {
            script_id: Uuid::new_v4(),
            expected_revision: 4,
            workspace: workspace(),
            actor_id: "operator:alloy-reviewer".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert!(command.validate().is_ok());
        let digest = command.request_digest().expect("candidate request digest");
        assert!(digest.starts_with("sha256:"));

        let mut changed = command.clone();
        changed
            .workspace
            .files
            .iter_mut()
            .find(|file| file.path == "src/lib.rs")
            .expect("guest source")
            .contents
            .push_str("\n// reviewed change\n");
        assert_ne!(
            command.request_digest().expect("original request digest"),
            changed.request_digest().expect("changed request digest")
        );
    }

    #[test]
    fn candidate_rejects_a_different_slug_or_non_monotonic_parent_version() {
        let parent = ArtifactReleaseRef {
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };
        let same_version = workspace();
        assert_eq!(
            validate_candidate_parent_release(&same_version, &parent),
            Err(RustComponentCandidateError::ParentReleaseMismatch)
        );
        let mut wrong_slug = parent.clone();
        wrong_slug.slug = "other_module".to_string();
        assert_eq!(
            validate_candidate_parent_release(&same_version, &wrong_slug),
            Err(RustComponentCandidateError::ParentReleaseMismatch)
        );
    }

    #[test]
    fn candidate_review_command_has_a_complete_idempotency_fingerprint() {
        let command = RustComponentCandidateReviewCommand {
            candidate_id: Uuid::new_v4(),
            status: ReviewStatus::Approved,
            policy_revision: "policy:evolution".to_string(),
            actor_id: "operator:reviewer".to_string(),
            reason: None,
            idempotency_key: Uuid::new_v4(),
        };
        assert!(command.validate().is_ok());
        let mut changed = command.clone();
        changed.reason = Some("Reviewed candidate source.".to_string());
        assert_ne!(
            command.request_digest().expect("original review digest"),
            changed.request_digest().expect("changed review digest")
        );
    }
}
