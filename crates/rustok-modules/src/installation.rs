use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value as SqlValue,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

use rustok_api::{
    ArtifactPermissionRegistration, ArtifactPermissionRegistrationPort,
    ArtifactPermissionRegistrationRequest, ArtifactPermissionScope,
};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::OutboxTransport;
use rustok_sandbox::{
    RhaiBindingInput, SandboxContext, SandboxPayload, SandboxPolicy, SandboxRequest, SandboxSubject,
};

use crate::{
    ArtifactDataError, ArtifactDataMigrationCheckpointStore, ArtifactModuleKind,
    ArtifactPayloadKind, ArtifactReleaseRef, ArtifactSandboxPolicyResolver,
    ModuleArtifactDescriptor, ModuleArtifactError, ModuleDependencyLockGraph, TrustPolicyRevision,
    TrustVerificationDecision, TrustVerificationRequest, TrustVerifier,
};

const RHAI_MEDIA_TYPE: &str = "application/vnd.rustok.rhai.source.v1";
const WASM_COMPONENT_MEDIA_TYPE: &str = "application/wasm";
const SIDECAR_MEDIA_TYPE: &str = "application/vnd.rustok.sidecar.v1";
const STATIC_PROMOTION_MEDIA_TYPE: &str = "application/vnd.rustok.static-promotion.v1";
const MAX_ARTIFACT_MIGRATION_CHECKPOINT_BYTES: usize = 16 * 1024;

/// Hard bounds applied before an artifact enters the admission pipeline. The
/// registry adapter must use them before downloading an OCI layer; the package
/// verification repeats them against the received bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAdmissionLimits {
    pub max_descriptor_bytes: u64,
    pub max_payload_bytes: u64,
}

impl Default for ArtifactAdmissionLimits {
    fn default() -> Self {
        Self {
            max_descriptor_bytes: 256 * 1024,
            max_payload_bytes: 64 * 1024 * 1024,
        }
    }
}

impl ArtifactAdmissionLimits {
    pub fn validate_descriptor_size(&self, actual: u64) -> Result<(), ModuleInstallationError> {
        if actual > self.max_descriptor_bytes {
            return Err(ModuleInstallationError::ArtifactTooLarge {
                kind: "descriptor",
                limit: self.max_descriptor_bytes,
                actual,
            });
        }
        Ok(())
    }

    pub fn validate_payload_size(&self, actual: u64) -> Result<(), ModuleInstallationError> {
        if actual > self.max_payload_bytes {
            return Err(ModuleInstallationError::ArtifactTooLarge {
                kind: "payload",
                limit: self.max_payload_bytes,
                actual,
            });
        }
        Ok(())
    }
}

/// A digest-pinned OCI manifest location. Tags are deliberately not part of the
/// install contract: an installation is always reproducible from an immutable
/// manifest digest. The executable layer digest remains in the descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciArtifactReference {
    pub registry: String,
    pub repository: String,
    pub digest: String,
}

impl OciArtifactReference {
    pub fn validate(&self) -> Result<(), ModuleInstallationError> {
        if self.registry.trim().is_empty()
            || self.registry.contains('/')
            || self.registry.chars().any(char::is_whitespace)
        {
            return Err(ModuleInstallationError::InvalidOciReference(
                "registry must be a non-empty OCI host".to_string(),
            ));
        }
        if self.repository.trim().is_empty()
            || self.repository.starts_with('/')
            || self.repository.ends_with('/')
            || self
                .repository
                .split('/')
                .any(|segment| segment.is_empty() || !valid_repository_segment(segment))
        {
            return Err(ModuleInstallationError::InvalidOciReference(
                "repository must be lowercase slash-separated OCI path segments".to_string(),
            ));
        }
        if !valid_digest(&self.digest) {
            return Err(ModuleInstallationError::InvalidOciReference(
                "digest must be a sha256 digest".to_string(),
            ));
        }
        Ok(())
    }

    pub fn canonical(&self) -> String {
        format!("{}/{}@{}", self.registry, self.repository, self.digest)
    }
}

/// Payload source resolved from an immutable OCI manifest. A temporary file
/// has already passed bounded streaming digest verification and is owned by
/// the package until admission consumes it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum ArtifactPayloadSource {
    Bytes(Vec<u8>),
    TemporaryFile(PathBuf),
}

/// Payload resolved from an OCI artifact manifest after layer selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleArtifactPackage {
    pub reference: OciArtifactReference,
    pub descriptor: ModuleArtifactDescriptor,
    pub media_type: String,
    pub payload: ArtifactPayloadSource,
}

impl ModuleArtifactPackage {
    /// Verifies artifact identity before it can enter a tenant or platform runtime.
    pub async fn verify(
        &self,
        limits: ArtifactAdmissionLimits,
    ) -> Result<(), ModuleInstallationError> {
        self.reference.validate()?;
        self.descriptor.validate()?;
        let (size, actual_digest) = match &self.payload {
            ArtifactPayloadSource::Bytes(bytes) => (bytes.len() as u64, sha256_digest(bytes)),
            ArtifactPayloadSource::TemporaryFile(path) => sha256_file(path).await?,
        };
        limits.validate_payload_size(size)?;
        if actual_digest != self.descriptor.artifact_digest {
            return Err(ModuleInstallationError::PayloadDigestMismatch {
                expected: self.descriptor.artifact_digest.clone(),
                actual: actual_digest,
            });
        }
        let expected_media_type = media_type_for(self.descriptor.payload_kind);
        if self.media_type != expected_media_type {
            return Err(ModuleInstallationError::UnexpectedMediaType {
                expected: expected_media_type.to_string(),
                actual: self.media_type.clone(),
            });
        }
        Ok(())
    }

    pub fn release_ref(&self) -> ArtifactReleaseRef {
        self.descriptor.release_ref()
    }
}

async fn sha256_file(path: &std::path::Path) -> Result<(u64, String), ModuleInstallationError> {
    use tokio::io::AsyncReadExt;
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
    let mut size = 0_u64;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .await
            .map_err(|error| ModuleInstallationError::Registry(error.to_string()))?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok((size, format!("sha256:{}", hex::encode(hasher.finalize()))))
}

/// The durable record owned by the module control plane after a package passed
/// identity verification. It is sufficient to construct an isolated execution
/// request without reopening the server's source or Cargo graph.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstalledModuleArtifact {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub reference: OciArtifactReference,
    pub release: ArtifactReleaseRef,
    pub descriptor: ModuleArtifactDescriptor,
    /// Exact resolved dependencies admitted with this installation. Runtime
    /// execution receives only this immutable graph, never a live registry
    /// lookup or a floating version constraint.
    pub dependency_lock: ModuleDependencyLockGraph,
    /// Exact capability grants selected by the control plane for this
    /// installation, independent of the artifact declaration and policy.
    pub capability_grant_revision: u64,
    pub installed_at: DateTime<Utc>,
}

/// Installation ownership boundary used by host persistence adapters to apply
/// platform-wide or tenant-scoped storage and row-level security.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ModuleInstallationScope {
    Platform,
    Tenant { tenant_id: Uuid },
}

/// Durable lifecycle state of an admitted installation. The initial admission
/// transaction persists `Admitted` at revision one; later lifecycle services
/// own the guarded transitions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactAdmissionStatus {
    Resolved,
    Verifying,
    Admitted,
    Installed,
    Active,
    Failed,
    Inactive,
    RolledBack,
}

impl ArtifactAdmissionStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Resolved => "resolved",
            Self::Verifying => "verifying",
            Self::Admitted => "admitted",
            Self::Installed => "installed",
            Self::Active => "active",
            Self::Failed => "failed",
            Self::Inactive => "inactive",
            Self::RolledBack => "rolled_back",
        }
    }
}

impl InstalledModuleArtifact {
    pub fn validate_dependency_lock(&self) -> Result<(), ModuleInstallationError> {
        self.dependency_lock
            .validate()
            .map_err(|error| ModuleInstallationError::DependencyLock(error.to_string()))?;
        let mut reachable = BTreeSet::new();
        let mut pending = self
            .descriptor
            .dependencies
            .iter()
            .map(|dependency| dependency.slug.as_str())
            .collect::<Vec<_>>();
        while let Some(slug) = pending.pop() {
            if !reachable.insert(slug) {
                continue;
            }
            let node = self
                .dependency_lock
                .nodes
                .iter()
                .find(|node| node.slug == slug)
                .ok_or_else(|| {
                    ModuleInstallationError::DependencyLock(format!(
                        "resolved graph does not select declared dependency `{slug}`"
                    ))
                })?;
            pending.extend(node.dependencies.iter().map(String::as_str));
        }
        if reachable.len() != self.dependency_lock.nodes.len() {
            return Err(ModuleInstallationError::DependencyLock(
                "resolved graph contains dependencies unreachable from the artifact descriptor"
                    .into(),
            ));
        }
        for dependency in &self.descriptor.dependencies {
            let node = self
                .dependency_lock
                .nodes
                .iter()
                .find(|node| node.slug == dependency.slug)
                .expect("declared dependencies were checked while walking the graph");
            let version = Version::parse(&node.version).map_err(|error| {
                ModuleInstallationError::DependencyLock(format!(
                    "resolved dependency `{}` has invalid version: {error}",
                    node.slug
                ))
            })?;
            let requirement =
                VersionReq::parse(&dependency.version_requirement).map_err(|error| {
                    ModuleInstallationError::DependencyLock(format!(
                        "declared dependency `{}` has invalid requirement: {error}",
                        dependency.slug
                    ))
                })?;
            if !requirement.matches(&version) {
                return Err(ModuleInstallationError::DependencyLock(format!(
                    "resolved dependency `{}` version `{version}` does not satisfy `{}`",
                    dependency.slug, dependency.version_requirement
                )));
            }
        }
        Ok(())
    }

    pub fn sandbox_request(
        &self,
        payload: Vec<u8>,
        context: SandboxContext,
        input: Value,
        policy: SandboxPolicy,
    ) -> Result<SandboxRequest, ModuleInstallationError> {
        self.validate_dependency_lock()?;
        let executor = self
            .descriptor
            .payload_kind
            .sandbox_executor()
            .ok_or(ModuleInstallationError::StaticPromotionRequired)?;
        for grant in &policy.grants {
            if !self.descriptor.capabilities.contains(&grant.name) {
                return Err(ModuleInstallationError::UndeclaredCapability(
                    grant.name.as_str().to_string(),
                ));
            }
        }
        let payload_digest = sha256_digest(&payload);
        if payload_digest != self.descriptor.artifact_digest {
            return Err(ModuleInstallationError::PayloadDigestMismatch {
                expected: self.descriptor.artifact_digest.clone(),
                actual: payload_digest,
            });
        }

        let input = if self.descriptor.payload_kind == ArtifactPayloadKind::Rhai {
            serde_json::to_value(RhaiBindingInput::new(input))
                .map_err(|error| ModuleInstallationError::RhaiBinding(error.to_string()))?
        } else {
            input
        };

        Ok(SandboxRequest {
            subject: SandboxSubject::ModuleArtifact {
                installation_id: self.installation_id,
                slug: self.release.slug.clone(),
                version: self.release.version.clone(),
                digest: self.release.digest.clone(),
            },
            context,
            payload: SandboxPayload {
                executor,
                media_type: media_type_for(self.descriptor.payload_kind).to_string(),
                digest: self.release.digest.clone(),
                runtime_abi: self.descriptor.runtime_abi.clone(),
                entrypoint: self.descriptor.entrypoint.clone(),
                bytes: payload,
            },
            input,
            policy,
        })
    }
}

#[async_trait]
pub trait ArtifactRegistry: Send + Sync {
    async fn fetch(
        &self,
        reference: &OciArtifactReference,
        limits: ArtifactAdmissionLimits,
    ) -> Result<ModuleArtifactPackage, ModuleInstallationError>;
}

/// Platform-owned content-addressed storage for admitted executable payloads.
#[async_trait]
pub trait ArtifactBlobStore: Send + Sync {
    async fn put_verified(&self, digest: &str, bytes: &[u8])
        -> Result<(), ModuleInstallationError>;
    async fn get_verified(&self, digest: &str) -> Result<Vec<u8>, ModuleInstallationError>;
}

/// Durable CAS publication is intentionally separate from the database
/// transaction. A reconciler closes the gap between `CasPublished` and a
/// committed admission/outbox record after process or storage failures.
#[async_trait]
pub trait DurableArtifactBlobStore: ArtifactBlobStore {
    async fn stage(
        &self,
        expected_digest: &str,
        expected_media_type: &str,
        bytes: &[u8],
    ) -> Result<StagedArtifactBlob, ModuleInstallationError>;
    /// Stages a verified temporary file without requiring the caller to first
    /// materialize it as a `Vec<u8>`. Drivers with native file/multipart
    /// uploads override this; the default preserves older test adapters.
    async fn stage_file(
        &self,
        expected_digest: &str,
        expected_media_type: &str,
        source: &std::path::Path,
    ) -> Result<StagedArtifactBlob, ModuleInstallationError> {
        let bytes = tokio::fs::read(source)
            .await
            .map_err(|error| ModuleInstallationError::Blob(error.to_string()))?;
        self.stage(expected_digest, expected_media_type, &bytes)
            .await
    }
    async fn publish(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError>;
    async fn discard(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError>;
    /// Returns only digests that have crossed the durable publish boundary.
    /// The reconciler compares this set with committed control-plane references.
    async fn published_digests(&self) -> Result<Vec<String>, ModuleInstallationError>;
    async fn delete(&self, digest: &str) -> Result<(), ModuleInstallationError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedArtifactBlob {
    pub stage_id: Uuid,
    pub digest: String,
    pub media_type: String,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactVerificationEvidence {
    pub manifest_digest: String,
    pub payload_digest: String,
    pub media_type: String,
    pub signer_identity: String,
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
    pub signature_verified: bool,
    pub provenance_verified: bool,
    pub sbom_verified: bool,
    pub evidence_references: Vec<String>,
    pub verified_at: DateTime<Utc>,
}

impl ArtifactVerificationEvidence {
    fn from_decision(
        artifact: &InstalledModuleArtifact,
        media_type: &str,
        decision: TrustVerificationDecision,
        verified_at: DateTime<Utc>,
    ) -> Self {
        Self {
            manifest_digest: artifact.reference.digest.clone(),
            payload_digest: artifact.descriptor.artifact_digest.clone(),
            media_type: media_type.to_string(),
            signer_identity: decision.signer_identity,
            trust_policy_revision: decision.trust_policy_revision,
            capability_policy_revision: decision.capability_policy_revision,
            signature_verified: decision.signature_verified,
            provenance_verified: decision.provenance_verified,
            sbom_verified: decision.sbom_verified,
            evidence_references: decision.evidence_references,
            verified_at,
        }
    }

    fn admitted(&self) -> bool {
        self.signature_verified && self.provenance_verified && self.sbom_verified
    }
}

/// A revision-guarded replacement of redacted trust evidence after a policy or
/// trust-root change. It never changes the admitted CAS digest or descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAdmissionReverification {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    pub evidence: ArtifactVerificationEvidence,
}

/// Immutable command envelope for a rollback selection. The caller supplies
/// the capability-grant revision produced by owner policy evaluation for the
/// predecessor release; the command never reopens an OCI registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRollbackRequest {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
    pub target_capability_grant_revision: u64,
    pub migration_rollback_mode: ArtifactMigrationRollbackMode,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactMigrationRollbackMode {
    Reversible,
    Compensating,
    Prohibited,
}

impl ArtifactMigrationRollbackMode {
    fn as_str(self) -> &'static str {
        match self {
            Self::Reversible => "reversible",
            Self::Compensating => "compensating",
            Self::Prohibited => "prohibited",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRollbackResult {
    pub operation_id: Uuid,
    pub target_installation_id: Uuid,
    pub source_revision: u64,
    pub target_revision: u64,
}

/// Removes runtime bindings for one active artifact selection while preserving
/// the admitted release, immutable evidence, rollback predecessor, and data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDeactivationRequest {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDeactivationResult {
    pub operation_id: Uuid,
    pub revision: u64,
}

/// Disables one tenant's intent for an admitted Optional artifact without
/// changing the installation, admission, runtime binding, or data state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTenantDisableRequest {
    pub installation_id: Uuid,
    pub tenant_id: Uuid,
    pub expected_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTenantDisableResult {
    pub revision: u64,
}

/// Re-enables one tenant's intent for an admitted Optional artifact without
/// changing its immutable installation, admission, runtime binding, or data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTenantEnableRequest {
    pub installation_id: Uuid,
    pub tenant_id: Uuid,
    pub expected_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTenantEnableResult {
    pub revision: u64,
}

#[derive(Debug)]
struct ArtifactTenantLifecycleCommand {
    installation_id: Uuid,
    tenant_id: Uuid,
    expected_revision: u64,
    actor_id: Uuid,
    reason: String,
    idempotency_key: Uuid,
    enabled: bool,
}

impl From<ArtifactTenantDisableRequest> for ArtifactTenantLifecycleCommand {
    fn from(request: ArtifactTenantDisableRequest) -> Self {
        Self {
            installation_id: request.installation_id,
            tenant_id: request.tenant_id,
            expected_revision: request.expected_revision,
            actor_id: request.actor_id,
            reason: request.reason,
            idempotency_key: request.idempotency_key,
            enabled: false,
        }
    }
}

impl From<ArtifactTenantEnableRequest> for ArtifactTenantLifecycleCommand {
    fn from(request: ArtifactTenantEnableRequest) -> Self {
        Self {
            installation_id: request.installation_id,
            tenant_id: request.tenant_id,
            expected_revision: request.expected_revision,
            actor_id: request.actor_id,
            reason: request.reason,
            idempotency_key: request.idempotency_key,
            enabled: true,
        }
    }
}

/// Removes an inactive artifact selection from one scope without deleting its
/// immutable evidence or CAS bytes. A separate retention/purge policy owns
/// physical deletion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactUninstallRequest {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactUninstallResult {
    pub operation_id: Uuid,
    pub revision: u64,
}

/// Revision-guarded durable record of data migration progress for an admitted
/// artifact. The checkpoint is owner metadata; untrusted payloads never write
/// installation state directly.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactMigrationCheckpointRequest {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    pub checkpoint: Value,
    pub has_irreversible_migration: bool,
}

/// Immutable owner command for one artifact admission. The actor and
/// idempotency key are control-plane facts; they are never supplied by the
/// untrusted OCI descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactAdmissionCommand {
    pub reference: OciArtifactReference,
    pub scope: ModuleInstallationScope,
    pub dependency_lock: ModuleDependencyLockGraph,
    /// Host-selected sandbox grants and limits. Descriptor declarations are
    /// validated against this policy but cannot create grants themselves.
    pub sandbox_policy: SandboxPolicy,
    pub actor_id: Uuid,
    pub idempotency_key: Uuid,
}

impl ArtifactAdmissionCommand {
    fn validate(&self) -> Result<(), ModuleInstallationError> {
        self.reference.validate()?;
        if self.actor_id.is_nil() || self.idempotency_key.is_nil() {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission requires non-nil actor and idempotency identities".into(),
            ));
        }
        if matches!(&self.scope, ModuleInstallationScope::Tenant { tenant_id } if tenant_id.is_nil())
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "tenant-scoped admission requires a non-nil tenant identity".into(),
            ));
        }
        self.dependency_lock
            .validate()
            .map_err(|error| ModuleInstallationError::DependencyLock(error.to_string()))?;
        Ok(())
    }

    fn request_digest(&self) -> Result<String, ModuleInstallationError> {
        #[derive(Serialize)]
        struct Fingerprint<'a> {
            reference: &'a OciArtifactReference,
            scope: &'a ModuleInstallationScope,
            dependency_lock: &'a ModuleDependencyLockGraph,
            sandbox_policy: &'a SandboxPolicy,
        }

        let bytes = serde_json::to_vec(&Fingerprint {
            reference: &self.reference,
            scope: &self.scope,
            dependency_lock: &self.dependency_lock,
            sandbox_policy: &self.sandbox_policy,
        })
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(sha256_digest(&bytes))
    }
}

/// Stable acknowledgement of one owner admission command. Retries return the
/// original installation identity and never create another release selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAdmissionResult {
    pub installation_id: Uuid,
    pub created: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactAdmissionStage {
    Staged,
    CasPublished,
    DbCommitted,
    Failed,
}

/// Durable DB/outbox adapter contract for the part that must be atomic.
#[async_trait]
pub trait ArtifactAdmissionStore: Send + Sync {
    async fn find_admission(
        &self,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<Option<ArtifactAdmissionResult>, ModuleInstallationError>;
    async fn commit_admission(
        &self,
        artifact: &InstalledModuleArtifact,
        staged: &StagedArtifactBlob,
        evidence: &ArtifactVerificationEvidence,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<ArtifactAdmissionResult, ModuleInstallationError>;
    async fn unfinished_admissions(
        &self,
    ) -> Result<Vec<ArtifactAdmissionRecoveryRecord>, ModuleInstallationError>;
    /// Returns the durable CAS digests retained by a committed installation.
    async fn referenced_blob_digests(&self) -> Result<BTreeSet<String>, ModuleInstallationError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAdmissionRecoveryRecord {
    pub staged: StagedArtifactBlob,
    pub stage: ArtifactAdmissionStage,
}

#[async_trait]
pub trait ArtifactBlobRetentionPolicy: Send + Sync {
    async fn may_delete(&self, digest: &str) -> Result<bool, ModuleInstallationError>;
}

/// Durable-policy input for one unreferenced CAS blob. References are checked
/// separately by the reconciler; this rule protects records retained for audit,
/// rollback, or legal hold even after their installation selection is removed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBlobRetentionRule {
    pub retain_until: DateTime<Utc>,
    pub legal_hold: bool,
    pub rollback_protected: bool,
    pub audit_retained: bool,
}

/// Small deterministic policy implementation for a loaded durable retention
/// snapshot. Production storage supplies the snapshot; CAS never infers a
/// deletion decision from object age alone. Missing snapshot data fails closed:
/// a collector needs an explicit eligible rule before it may delete a blob.
pub struct SnapshotArtifactBlobRetentionPolicy {
    now: DateTime<Utc>,
    rules: HashMap<String, ArtifactBlobRetentionRule>,
}

impl SnapshotArtifactBlobRetentionPolicy {
    pub fn new(now: DateTime<Utc>, rules: HashMap<String, ArtifactBlobRetentionRule>) -> Self {
        Self { now, rules }
    }
}

#[async_trait]
impl ArtifactBlobRetentionPolicy for SnapshotArtifactBlobRetentionPolicy {
    async fn may_delete(&self, digest: &str) -> Result<bool, ModuleInstallationError> {
        Ok(match self.rules.get(digest) {
            None => false,
            Some(rule) => {
                !rule.legal_hold
                    && !rule.rollback_protected
                    && !rule.audit_retained
                    && rule.retain_until <= self.now
            }
        })
    }
}

/// Reconciles temporary uploads and published CAS blobs. A published blob is
/// removed only when no committed installation references it and the retention
/// policy explicitly permits deletion.
pub struct ArtifactAdmissionReconciler<B, S> {
    blobs: B,
    admissions: S,
}

impl<B, S> ArtifactAdmissionReconciler<B, S>
where
    B: DurableArtifactBlobStore,
    S: ArtifactAdmissionStore,
{
    pub fn new(blobs: B, admissions: S) -> Self {
        Self { blobs, admissions }
    }

    pub async fn discard_unpublished_staging(&self) -> Result<usize, ModuleInstallationError> {
        let records = self.admissions.unfinished_admissions().await?;
        let mut discarded = 0;
        for record in records {
            if record.stage == ArtifactAdmissionStage::Staged {
                self.blobs.discard(&record.staged).await?;
                discarded += 1;
            }
        }
        Ok(discarded)
    }

    pub async fn delete_unreferenced_published(
        &self,
        retention: &dyn ArtifactBlobRetentionPolicy,
    ) -> Result<usize, ModuleInstallationError> {
        let referenced = self.admissions.referenced_blob_digests().await?;
        let mut deleted = 0;
        for digest in self.blobs.published_digests().await? {
            if !referenced.contains(&digest) && retention.may_delete(&digest).await? {
                self.blobs.delete(&digest).await?;
                deleted += 1;
            }
        }
        Ok(deleted)
    }
}

/// Test/local adapter. Production adapters must use durable storage outside
/// PostgreSQL and preserve the same digest verification invariant.
#[derive(Default)]
pub struct InMemoryArtifactBlobStore {
    blobs: Mutex<HashMap<String, Vec<u8>>>,
    staged: Mutex<HashMap<Uuid, InMemoryStagedArtifact>>,
}

struct InMemoryStagedArtifact {
    bytes: Vec<u8>,
}

#[async_trait]
impl ArtifactBlobStore for InMemoryArtifactBlobStore {
    async fn put_verified(
        &self,
        digest: &str,
        bytes: &[u8],
    ) -> Result<(), ModuleInstallationError> {
        if sha256_digest(bytes) != digest {
            return Err(ModuleInstallationError::PayloadDigestMismatch {
                expected: digest.to_string(),
                actual: sha256_digest(bytes),
            });
        }
        self.blobs
            .lock()
            .map_err(|_| ModuleInstallationError::Blob("blob store lock poisoned".into()))?
            .insert(digest.to_string(), bytes.to_vec());
        Ok(())
    }

    async fn get_verified(&self, digest: &str) -> Result<Vec<u8>, ModuleInstallationError> {
        let bytes = self
            .blobs
            .lock()
            .map_err(|_| ModuleInstallationError::Blob("blob store lock poisoned".into()))?
            .get(digest)
            .cloned()
            .ok_or_else(|| ModuleInstallationError::BlobNotFound(digest.to_string()))?;
        if sha256_digest(&bytes) != digest {
            return Err(ModuleInstallationError::Blob(
                "stored blob digest mismatch".into(),
            ));
        }
        Ok(bytes)
    }
}

#[async_trait]
impl DurableArtifactBlobStore for InMemoryArtifactBlobStore {
    async fn stage(
        &self,
        expected_digest: &str,
        expected_media_type: &str,
        bytes: &[u8],
    ) -> Result<StagedArtifactBlob, ModuleInstallationError> {
        if expected_media_type.trim().is_empty() {
            return Err(ModuleInstallationError::Blob(
                "artifact media type is empty".into(),
            ));
        }
        if sha256_digest(bytes) != expected_digest {
            return Err(ModuleInstallationError::PayloadDigestMismatch {
                expected: expected_digest.to_string(),
                actual: sha256_digest(bytes),
            });
        }
        let staged = StagedArtifactBlob {
            stage_id: Uuid::new_v4(),
            digest: expected_digest.to_string(),
            media_type: expected_media_type.to_string(),
            size_bytes: bytes.len() as u64,
        };
        self.staged
            .lock()
            .map_err(|_| ModuleInstallationError::Blob("blob store lock poisoned".into()))?
            .insert(
                staged.stage_id,
                InMemoryStagedArtifact {
                    bytes: bytes.to_vec(),
                },
            );
        Ok(staged)
    }

    async fn publish(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError> {
        let stored = self
            .staged
            .lock()
            .map_err(|_| ModuleInstallationError::Blob("blob store lock poisoned".into()))?
            .remove(&staged.stage_id)
            .ok_or_else(|| ModuleInstallationError::Blob("staged blob is unavailable".into()))?;
        self.put_verified(&staged.digest, &stored.bytes).await
    }

    async fn discard(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError> {
        self.staged
            .lock()
            .map_err(|_| ModuleInstallationError::Blob("blob store lock poisoned".into()))?
            .remove(&staged.stage_id);
        Ok(())
    }

    async fn published_digests(&self) -> Result<Vec<String>, ModuleInstallationError> {
        Ok(self
            .blobs
            .lock()
            .map_err(|_| ModuleInstallationError::Blob("blob store lock poisoned".into()))?
            .keys()
            .cloned()
            .collect())
    }

    async fn delete(&self, digest: &str) -> Result<(), ModuleInstallationError> {
        self.blobs
            .lock()
            .map_err(|_| ModuleInstallationError::Blob("blob store lock poisoned".into()))?
            .remove(digest);
        Ok(())
    }
}

/// SeaORM adapter for the module-owned installation and admission tables. The
/// OCI payload is deliberately not copied into PostgreSQL: CAS owns the bytes.
#[derive(Clone)]
pub struct SeaOrmArtifactInstallationStore {
    db: DatabaseConnection,
}

/// Resolves the host-owned durable sandbox policy for one exact admitted
/// artifact installation. Descriptor capabilities are declarations only; a
/// policy must explicitly grant them before the sandbox can use a host
/// capability.
#[derive(Clone)]
pub struct SeaOrmArtifactSandboxPolicyResolver {
    db: DatabaseConnection,
}

impl SeaOrmArtifactSandboxPolicyResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

impl SeaOrmArtifactInstallationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Resolves the exact active installation named by a platform-routed binding.
    /// The initial identity lookup is not trusted for execution: it is reduced
    /// to an immutable release tuple and then rechecked through `resolve_exact`.
    pub async fn resolve_routed_installation(
        &self,
        installation_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<InstalledModuleArtifact, String> {
        if installation_id.is_nil() || tenant_id.is_nil() {
            return Err("artifact route requires non-nil identities".to_string());
        }
        let transaction = self.db.begin().await.map_err(|error| error.to_string())?;
        configure_rls_scope(&transaction, &ModuleInstallationScope::Tenant { tenant_id })
            .await
            .map_err(|error| error.to_string())?;
        let backend = transaction.get_database_backend();
        let placeholder = if backend == DbBackend::Postgres {
            "$1"
        } else {
            "?1"
        };
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT slug, version, payload_digest FROM module_artifact_installations WHERE installation_id = {placeholder} LIMIT 1"
                ),
                vec![uuid_value(installation_id, backend)],
            ))
            .await
            .map_err(|error| error.to_string())?;
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;
        let row = row.ok_or_else(|| "artifact routed installation is unavailable".to_string())?;
        let release = ArtifactReleaseRef {
            slug: row.try_get("", "slug").map_err(|error| error.to_string())?,
            version: row
                .try_get("", "version")
                .map_err(|error| error.to_string())?,
            digest: row
                .try_get("", "payload_digest")
                .map_err(|error| error.to_string())?,
        };
        <Self as crate::ArtifactInstallationResolver>::resolve_exact(
            self,
            installation_id,
            &release,
            tenant_id,
        )
        .await
    }

    /// Persists a data-migration checkpoint with the admission revision CAS.
    /// Once an installation records an irreversible migration, later commands
    /// cannot clear that fact by submitting a false flag.
    pub async fn record_migration_checkpoint(
        &self,
        request: ArtifactMigrationCheckpointRequest,
    ) -> Result<u64, ModuleInstallationError> {
        validate_migration_checkpoint_request(&request)?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = transaction.get_database_backend();
        let (scope_kind, tenant_id) = match request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(tenant_id)),
        };
        let scope = match backend {
            DbBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            _ => "installation.scope_kind = ?2 AND installation.tenant_id IS ?3",
        };
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT admission.revision, admission.status, installation.has_irreversible_migration \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {} AND {scope} \
                     AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                     WHERE uninstall.installation_id = installation.installation_id)",
                    if backend == DbBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    uuid_value(request.installation_id, backend),
                    scope_kind.into(),
                    optional_uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| ModuleInstallationError::AdmissionRevisionConflict("installation is absent, uninstalled, or outside the requested scope".into()))?;
        let revision: i64 = row
            .try_get("", "revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let status: String = row
            .try_get("", "status")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let has_irreversible_migration = match backend {
            DbBackend::Postgres => row
                .try_get::<bool>("", "has_irreversible_migration")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            _ => {
                row.try_get::<i64>("", "has_irreversible_migration")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                    != 0
            }
        };
        if revision != request.expected_revision as i64
            || !matches!(status.as_str(), "admitted" | "installed" | "inactive")
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "migration checkpoint requires an eligible installation at the expected revision"
                    .into(),
            ));
        }
        let checkpoint = serde_json::to_value(&request.checkpoint)
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let update_installation = match backend {
            DbBackend::Postgres => "UPDATE module_artifact_installations SET migration_checkpoint = $1, has_irreversible_migration = has_irreversible_migration OR $2 WHERE installation_id = $3",
            _ => "UPDATE module_artifact_installations SET migration_checkpoint = ?1, has_irreversible_migration = MAX(has_irreversible_migration, ?2) WHERE installation_id = ?3",
        };
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                update_installation.to_string(),
                vec![
                    SqlValue::Json(Some(Box::new(checkpoint))),
                    request.has_irreversible_migration.into(),
                    uuid_value(request.installation_id, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let update_admission = format!("UPDATE module_artifact_admissions SET revision = revision + 1 WHERE installation_id = {} AND revision = {}", if backend == DbBackend::Postgres { "$1" } else { "?1" }, if backend == DbBackend::Postgres { "$2" } else { "?2" });
        if transaction
            .execute(Statement::from_sql_and_values(
                backend,
                update_admission,
                vec![
                    uuid_value(request.installation_id, backend),
                    revision.into(),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .rows_affected()
            != 1
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "installation became stale while recording a migration checkpoint".into(),
            ));
        }
        let tenant_id = match &request.scope {
            ModuleInstallationScope::Platform => None,
            ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
        };
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    tenant_id,
                    DomainEvent::ModuleArtifactMigrationCheckpointed {
                        installation_id: request.installation_id,
                        revision: request.expected_revision + 1,
                        has_irreversible_migration: has_irreversible_migration
                            || request.has_irreversible_migration,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleInstallationError::Outbox(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(request.expected_revision + 1)
    }

    /// Deactivates runtime bindings for an active selection without deleting
    /// the admitted release or its rollback/data evidence.
    pub async fn deactivate_artifact(
        &self,
        request: ArtifactDeactivationRequest,
    ) -> Result<ArtifactDeactivationResult, ModuleInstallationError> {
        validate_lifecycle_command(
            request.installation_id,
            &request.scope,
            request.expected_revision,
            request.actor_id,
            &request.reason,
            request.idempotency_key,
        )?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = transaction.get_database_backend();
        let (scope_kind, tenant_id) = match request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(tenant_id)),
        };
        let scope = match backend {
            DbBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            _ => "installation.scope_kind = ?2 AND installation.tenant_id IS ?3",
        };
        let existing = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT operation.operation_id, operation.installation_id, \
                     operation.expected_revision, operation.actor_id, operation.reason \
                     FROM module_artifact_deactivation_operations operation \
                     JOIN module_artifact_installations installation ON installation.installation_id = operation.installation_id \
                     WHERE operation.idempotency_key = {} AND {scope}",
                    if backend == DbBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    uuid_value(request.idempotency_key, backend),
                    scope_kind.into(),
                    optional_uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if let Some(existing) = existing {
            let installation_id = required_uuid_from_row(&existing, "installation_id", backend)?;
            let expected: i64 = existing
                .try_get("", "expected_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let actor_id = required_uuid_from_row(&existing, "actor_id", backend)?;
            let reason: String = existing
                .try_get("", "reason")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if installation_id != request.installation_id
                || expected != request.expected_revision as i64
                || actor_id != request.actor_id
                || reason != request.reason
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "idempotency key was already used for a different deactivation command".into(),
                ));
            }
            let operation_id = required_uuid_from_row(&existing, "operation_id", backend)?;
            return Ok(ArtifactDeactivationResult {
                operation_id,
                revision: request.expected_revision + 1,
            });
        }
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT installation.slug, admission.status, admission.revision \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {} AND {scope} \
                     AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                     WHERE uninstall.installation_id = installation.installation_id)",
                    if backend == DbBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    uuid_value(request.installation_id, backend),
                    scope_kind.into(),
                    optional_uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "installation is absent, uninstalled, or outside the requested scope".into(),
                )
            })?;
        let status: String = row
            .try_get("", "status")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let revision: i64 = row
            .try_get("", "revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if status != "active" || revision != request.expected_revision as i64 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "installation must be active at the expected revision before deactivation".into(),
            ));
        }
        let target_slug: String = row
            .try_get("", "slug")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let dependents = transaction
            .query_all(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id <> {} AND {scope} AND admission.status = 'active'",
                    if backend == DbBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    uuid_value(request.installation_id, backend),
                    scope_kind.into(),
                    optional_uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        for dependent in dependents {
            let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
                &dependent
                    .try_get::<String>("", "descriptor")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            )
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if descriptor
                .dependencies
                .iter()
                .any(|dependency| dependency.slug == target_slug)
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "deactivation is blocked by an active dependent in the same scope".into(),
                ));
            }
        }
        let operation_id = Uuid::new_v4();
        let placeholders = match backend {
            DbBackend::Postgres => "$1,$2,$3,$4,$5,$6,NOW()",
            _ => "?1,?2,?3,?4,?5,?6,datetime('now')",
        };
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_deactivation_operations \
                     (operation_id, installation_id, expected_revision, actor_id, reason, idempotency_key, committed_at) \
                     VALUES ({placeholders})"
                ),
                vec![
                    uuid_value(operation_id, backend),
                    uuid_value(request.installation_id, backend),
                    revision.into(),
                    uuid_value(request.actor_id, backend),
                    request.reason.into(),
                    uuid_value(request.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_admissions SET status = 'inactive', revision = revision + 1 \
                     WHERE installation_id = {} AND revision = {} AND status = 'active'",
                    if backend == DbBackend::Postgres { "$1" } else { "?1" },
                    if backend == DbBackend::Postgres { "$2" } else { "?2" },
                ),
                vec![uuid_value(request.installation_id, backend), revision.into()],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if updated.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "installation became stale during deactivation".into(),
            ));
        }
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    tenant_id,
                    DomainEvent::ModuleArtifactDeactivated {
                        installation_id: request.installation_id,
                        revision: request.expected_revision + 1,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleInstallationError::Outbox(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(ArtifactDeactivationResult {
            operation_id,
            revision: request.expected_revision + 1,
        })
    }

    pub async fn disable_artifact_for_tenant(
        &self,
        request: ArtifactTenantDisableRequest,
    ) -> Result<ArtifactTenantDisableResult, ModuleInstallationError> {
        self.set_artifact_tenant_enabled(request.into())
            .await
            .map(|revision| ArtifactTenantDisableResult { revision })
    }

    /// Restores an Optional artifact's tenant intent through the same
    /// revision-CAS, idempotency, audit, and outbox path as disable.
    pub async fn enable_artifact_for_tenant(
        &self,
        request: ArtifactTenantEnableRequest,
    ) -> Result<ArtifactTenantEnableResult, ModuleInstallationError> {
        self.set_artifact_tenant_enabled(request.into())
            .await
            .map(|revision| ArtifactTenantEnableResult { revision })
    }

    async fn set_artifact_tenant_enabled(
        &self,
        request: ArtifactTenantLifecycleCommand,
    ) -> Result<u64, ModuleInstallationError> {
        let scope = ModuleInstallationScope::Tenant {
            tenant_id: request.tenant_id,
        };
        validate_lifecycle_command(
            request.installation_id,
            &scope,
            request.expected_revision,
            request.actor_id,
            &request.reason,
            request.idempotency_key,
        )?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &scope).await?;
        let backend = transaction.get_database_backend();
        let installation_placeholder = if backend == DbBackend::Postgres {
            "$1"
        } else {
            "?1"
        };
        let tenant_scope = match backend {
            DbBackend::Postgres => {
                "installation.scope_kind = 'platform' OR \
                 (installation.scope_kind = 'tenant' AND installation.tenant_id = $2)"
            }
            _ => {
                "installation.scope_kind = 'platform' OR \
                 (installation.scope_kind = 'tenant' AND installation.tenant_id = ?2)"
            }
        };
        let artifact = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {installation_placeholder} \
                       AND ({tenant_scope}) \
                       AND admission.status IN ('admitted', 'installed', 'active', 'inactive', 'rolled_back')",
                ),
                vec![
                    uuid_value(request.installation_id, backend),
                    uuid_value(request.tenant_id, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "artifact is unavailable in the requested tenant scope".into(),
                )
            })?;
        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
            &artifact
                .try_get::<String>("", "descriptor")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        )
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if descriptor.module_kind != ArtifactModuleKind::Optional {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "tenant lifecycle changes are allowed only for Optional artifacts".into(),
            ));
        }
        let placeholder = if backend == DbBackend::Postgres {
            "$1"
        } else {
            "?1"
        };
        let tenant_placeholder = if backend == DbBackend::Postgres {
            "$2"
        } else {
            "?2"
        };
        let existing = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT enabled, revision, expected_revision, idempotency_key, actor_id, reason \
                     FROM module_artifact_tenant_lifecycle \
                     WHERE installation_id = {placeholder} AND tenant_id = {tenant_placeholder}"
                ),
                vec![
                    uuid_value(request.installation_id, backend),
                    uuid_value(request.tenant_id, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let revision = if let Some(row) = existing {
            let current_enabled = match backend {
                DbBackend::Postgres => row
                    .try_get::<bool>("", "enabled")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
                _ => {
                    row.try_get::<i64>("", "enabled")
                        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                        != 0
                }
            };
            let current: i64 = row
                .try_get("", "revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let expected_revision: i64 = row
                .try_get("", "expected_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let idempotency_key = match backend {
                DbBackend::Postgres => row
                    .try_get::<Uuid>("", "idempotency_key")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
                _ => row
                    .try_get::<String>("", "idempotency_key")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                    .parse::<Uuid>()
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            };
            if idempotency_key == request.idempotency_key {
                let actor_id = required_uuid_from_row(&row, "actor_id", backend)?;
                let reason: String = row
                    .try_get("", "reason")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                if current_enabled != request.enabled
                    || expected_revision != request.expected_revision as i64
                    || actor_id != request.actor_id
                    || reason != request.reason
                {
                    return Err(ModuleInstallationError::AdmissionRevisionConflict(
                        "idempotency key was already used for a different tenant lifecycle command"
                            .into(),
                    ));
                }
                transaction
                    .commit()
                    .await
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                return u64::try_from(current).map_err(|_| {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "tenant lifecycle revision is outside the supported range".into(),
                    )
                });
            }
            if current != request.expected_revision as i64 {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "tenant lifecycle revision is stale".into(),
                ));
            }
            let next_revision = u64::try_from(current)
                .ok()
                .and_then(|revision| revision.checked_add(1))
                .ok_or_else(|| {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "tenant lifecycle revision is outside the supported range".into(),
                    )
                })?;
            let placeholders = match backend {
                DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5", "$6", "$7"),
                _ => ("?1", "?2", "?3", "?4", "?5", "?6", "?7"),
            };
            let enabled = if request.enabled { "TRUE" } else { "FALSE" };
            let updated = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_tenant_lifecycle \
                         SET enabled = {enabled}, revision = revision + 1, expected_revision = {}, \
                             idempotency_key = {}, actor_id = {}, reason = {} \
                         WHERE installation_id = {} AND tenant_id = {} AND revision = {}",
                        placeholders.0,
                        placeholders.1,
                        placeholders.2,
                        placeholders.3,
                        placeholders.4,
                        placeholders.5,
                        placeholders.6,
                    ),
                    vec![
                        (request.expected_revision as i64).into(),
                        uuid_value(request.idempotency_key, backend),
                        uuid_value(request.actor_id, backend),
                        request.reason.into(),
                        uuid_value(request.installation_id, backend),
                        uuid_value(request.tenant_id, backend),
                        current.into(),
                    ],
                ))
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if updated.rows_affected() != 1 {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "tenant lifecycle became stale during update".into(),
                ));
            }
            next_revision
        } else {
            if request.expected_revision != 1 {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "new tenant lifecycle state starts at revision 1".into(),
                ));
            }
            let enabled = if request.enabled { "TRUE" } else { "FALSE" };
            let insert_sql = match backend {
                DbBackend::Postgres => format!(
                    "INSERT INTO module_artifact_tenant_lifecycle \
                     (installation_id, tenant_id, enabled, revision, expected_revision, idempotency_key, actor_id, reason, updated_at) \
                     VALUES ($1, $2, {enabled}, 1, $3, $4, $5, $6, NOW())"
                ),
                _ => format!(
                    "INSERT INTO module_artifact_tenant_lifecycle \
                     (installation_id, tenant_id, enabled, revision, expected_revision, idempotency_key, actor_id, reason, updated_at) \
                     VALUES (?1, ?2, {enabled}, 1, ?3, ?4, ?5, ?6, datetime('now'))"
                ),
            };
            transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    insert_sql,
                    vec![
                        uuid_value(request.installation_id, backend),
                        uuid_value(request.tenant_id, backend),
                        (request.expected_revision as i64).into(),
                        uuid_value(request.idempotency_key, backend),
                        uuid_value(request.actor_id, backend),
                        request.reason.into(),
                    ],
                ))
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            1
        };
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    Some(request.tenant_id),
                    if request.enabled {
                        DomainEvent::ModuleArtifactTenantEnabled {
                            installation_id: request.installation_id,
                            tenant_id: request.tenant_id,
                            revision,
                        }
                    } else {
                        DomainEvent::ModuleArtifactTenantDisabled {
                            installation_id: request.installation_id,
                            tenant_id: request.tenant_id,
                            revision,
                        }
                    },
                ),
            )
            .await
            .map_err(|error| ModuleInstallationError::Outbox(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(revision)
    }

    /// Removes one inactive scope selection while retaining immutable evidence
    /// for rollback/audit and deferred CAS retention.
    pub async fn uninstall_artifact(
        &self,
        request: ArtifactUninstallRequest,
    ) -> Result<ArtifactUninstallResult, ModuleInstallationError> {
        validate_lifecycle_command(
            request.installation_id,
            &request.scope,
            request.expected_revision,
            request.actor_id,
            &request.reason,
            request.idempotency_key,
        )?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = transaction.get_database_backend();
        let (scope_kind, tenant_id) = match request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(tenant_id)),
        };
        let scope = match backend {
            DbBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            _ => "installation.scope_kind = ?2 AND installation.tenant_id IS ?3",
        };
        let existing = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT uninstall.operation_id, uninstall.installation_id, \
                     uninstall.expected_revision, uninstall.actor_id, uninstall.reason \
                     FROM module_artifact_uninstall_operations uninstall \
                     JOIN module_artifact_installations installation ON installation.installation_id = uninstall.installation_id \
                     WHERE uninstall.idempotency_key = {} AND {scope}",
                    if backend == DbBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    uuid_value(request.idempotency_key, backend),
                    scope_kind.into(),
                    optional_uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        if let Some(existing) = existing {
            let installation_id = required_uuid_from_row(&existing, "installation_id", backend)?;
            let expected: i64 = existing
                .try_get("", "expected_revision")
                .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
            let actor_id = required_uuid_from_row(&existing, "actor_id", backend)?;
            let reason: String = existing
                .try_get("", "reason")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if installation_id != request.installation_id
                || expected != request.expected_revision as i64
                || actor_id != request.actor_id
                || reason != request.reason
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "idempotency key was already used for a different uninstall command".into(),
                ));
            }
            let operation_id = required_uuid_from_row(&existing, "operation_id", backend)?;
            return Ok(ArtifactUninstallResult {
                operation_id,
                revision: request.expected_revision + 1,
            });
        }
        let row = transaction.query_one(Statement::from_sql_and_values(backend, format!(
            "SELECT installation.slug, admission.status, admission.revision FROM module_artifact_installations installation JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id WHERE installation.installation_id = {} AND {scope}", if backend == DbBackend::Postgres { "$1" } else { "?1" }), vec![uuid_value(request.installation_id, backend), scope_kind.into(), optional_uuid_value(tenant_id, backend)])).await.map_err(|e| ModuleInstallationError::Store(e.to_string()))?.ok_or_else(|| ModuleInstallationError::AdmissionRevisionConflict("installation is absent from the requested scope".into()))?;
        let status: String = row
            .try_get("", "status")
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        let revision: i64 = row
            .try_get("", "revision")
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        if status != "inactive" || revision != request.expected_revision as i64 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "installation must be inactive at the expected revision before uninstall".into(),
            ));
        }
        let target_slug: String = row
            .try_get("", "slug")
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        let dependents = transaction
            .query_all(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT installation.slug, CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id <> {} AND {scope} AND admission.status = 'active'",
                    if backend == DbBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    uuid_value(request.installation_id, backend),
                    scope_kind.into(),
                    optional_uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        for dependent in dependents {
            let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
                &dependent
                    .try_get::<String>("", "descriptor")
                    .map_err(|e| ModuleInstallationError::Store(e.to_string()))?,
            )
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
            if descriptor
                .dependencies
                .iter()
                .any(|dependency| dependency.slug == target_slug)
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "uninstall is blocked by an active dependent in the same scope".into(),
                ));
            }
        }
        let operation_id = Uuid::new_v4();
        let p = if backend == DbBackend::Postgres {
            "$1,$2,$3,$4,$5,$6,NOW()"
        } else {
            "?1,?2,?3,?4,?5,?6,datetime('now')"
        };
        transaction.execute(Statement::from_sql_and_values(backend, format!("INSERT INTO module_artifact_uninstall_operations (operation_id, installation_id, expected_revision, actor_id, reason, idempotency_key, committed_at) VALUES ({p})"), vec![uuid_value(operation_id, backend), uuid_value(request.installation_id, backend), revision.into(), uuid_value(request.actor_id, backend), request.reason.into(), uuid_value(request.idempotency_key, backend)])).await.map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        let updated = transaction.execute(Statement::from_sql_and_values(backend, format!("UPDATE module_artifact_admissions SET revision = revision + 1 WHERE installation_id = {} AND revision = {} AND status = 'inactive'", if backend == DbBackend::Postgres { "$1" } else { "?1" }, if backend == DbBackend::Postgres { "$2" } else { "?2" }), vec![uuid_value(request.installation_id, backend), revision.into()])).await.map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        if updated.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "installation became stale during uninstall".into(),
            ));
        }
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    tenant_id,
                    DomainEvent::ModuleArtifactUninstalled {
                        installation_id: request.installation_id,
                        revision: request.expected_revision + 1,
                    },
                ),
            )
            .await
            .map_err(|e| ModuleInstallationError::Outbox(e.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        Ok(ArtifactUninstallResult {
            operation_id,
            revision: request.expected_revision + 1,
        })
    }

    /// Applies the durable selection half of a rollback. Runtime activation is
    /// deliberately downstream of this transaction.
    pub async fn rollback_artifact(
        &self,
        request: ArtifactRollbackRequest,
    ) -> Result<ArtifactRollbackResult, ModuleInstallationError> {
        if request.expected_revision == 0
            || request.target_capability_grant_revision == 0
            || request.reason.trim().is_empty()
            || request.actor_id.is_nil()
            || request.idempotency_key.is_nil()
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback requires positive revisions, non-nil identities, and a non-empty reason"
                    .into(),
            ));
        }
        if matches!(&request.scope, ModuleInstallationScope::Tenant { tenant_id } if tenant_id.is_nil())
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "tenant-scoped rollback requires a non-nil tenant identity".into(),
            ));
        }
        let expected_revision = i64::try_from(request.expected_revision).map_err(|_| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "rollback revision exceeds database range".into(),
            )
        })?;
        let target_capability_grant_revision =
            i64::try_from(request.target_capability_grant_revision).map_err(|_| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "capability grant revision exceeds database range".into(),
                )
            })?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = transaction.get_database_backend();
        let (scope_kind, tenant_id) = match &request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
        };
        let scope = match backend {
            DbBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            _ => "installation.scope_kind = ?2 AND installation.tenant_id IS ?3",
        };
        let existing = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT operation.operation_id, operation.installation_id, \
                     operation.target_installation_id, operation.expected_revision, \
                     operation.actor_id, operation.reason, \
                     operation.target_capability_grant_revision, \
                     operation.migration_rollback_mode, operation.source_revision, \
                     operation.target_revision \
                     FROM module_artifact_rollback_operations operation \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = operation.installation_id \
                     WHERE operation.idempotency_key = {} AND {scope}",
                    if backend == DbBackend::Postgres {
                        "$1"
                    } else {
                        "?1"
                    }
                ),
                vec![
                    uuid_value(request.idempotency_key, backend),
                    scope_kind.into(),
                    optional_uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if let Some(existing) = existing {
            let installation_id = required_uuid_from_row(&existing, "installation_id", backend)?;
            let target_installation_id =
                required_uuid_from_row(&existing, "target_installation_id", backend)?;
            let stored_expected_revision: i64 = existing
                .try_get("", "expected_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let actor_id = required_uuid_from_row(&existing, "actor_id", backend)?;
            let reason: String = existing
                .try_get("", "reason")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let stored_grant_revision: Option<i64> = existing
                .try_get("", "target_capability_grant_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let stored_mode: Option<String> = existing
                .try_get("", "migration_rollback_mode")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let source_revision: Option<i64> = existing
                .try_get("", "source_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let target_revision: Option<i64> = existing
                .try_get("", "target_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if installation_id != request.installation_id
                || stored_expected_revision != expected_revision
                || actor_id != request.actor_id
                || reason != request.reason
                || stored_grant_revision != Some(target_capability_grant_revision)
                || stored_mode.as_deref() != Some(request.migration_rollback_mode.as_str())
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "idempotency key was already used for a different rollback command".into(),
                ));
            }
            let source_revision = source_revision.ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "rollback idempotency record lacks an immutable response fingerprint".into(),
                )
            })?;
            let target_revision = target_revision.ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "rollback idempotency record lacks an immutable response fingerprint".into(),
                )
            })?;
            return Ok(ArtifactRollbackResult {
                operation_id: required_uuid_from_row(&existing, "operation_id", backend)?,
                target_installation_id,
                source_revision: u64::try_from(source_revision).map_err(|_| {
                    ModuleInstallationError::Store(
                        "rollback source revision is negative".to_string(),
                    )
                })?,
                target_revision: u64::try_from(target_revision).map_err(|_| {
                    ModuleInstallationError::Store(
                        "rollback target revision is negative".to_string(),
                    )
                })?,
            });
        }
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2"),
            _ => ("?1", "?2"),
        };
        let row = transaction.query_one(Statement::from_sql_and_values(
            backend,
            format!("SELECT previous_installation_id, has_irreversible_migration FROM module_artifact_installations WHERE installation_id = {}", placeholders.0),
            vec![uuid_value(request.installation_id, backend)],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| ModuleInstallationError::AdmissionRevisionConflict("rollback predecessor is unavailable".into()))?;
        let target_installation_id = match backend {
            DbBackend::Postgres => row
                .try_get("", "previous_installation_id")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            _ => row
                .try_get::<String>("", "previous_installation_id")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                .parse::<Uuid>()
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        };
        let has_irreversible_migration = match backend {
            DbBackend::Postgres => row
                .try_get::<bool>("", "has_irreversible_migration")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            _ => {
                row.try_get::<i64>("", "has_irreversible_migration")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                    != 0
            }
        };
        if matches!(
            request.migration_rollback_mode,
            ArtifactMigrationRollbackMode::Prohibited
        ) || (has_irreversible_migration
            && !matches!(
                request.migration_rollback_mode,
                ArtifactMigrationRollbackMode::Compensating
            ))
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback is prohibited by the recorded data-migration policy".into(),
            ));
        }
        let target_revision_row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT revision FROM module_artifact_admissions WHERE installation_id = {}",
                    placeholders.0
                ),
                vec![uuid_value(target_installation_id, backend)],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "rollback predecessor admission is unavailable".into(),
                )
            })?;
        let target_expected_revision: i64 = target_revision_row
            .try_get("", "revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let source_revision = expected_revision.checked_add(1).ok_or_else(|| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "rollback source revision exceeds database range".into(),
            )
        })?;
        let target_revision = target_expected_revision.checked_add(1).ok_or_else(|| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "rollback target revision exceeds database range".into(),
            )
        })?;
        let operation_id = Uuid::new_v4();
        transaction.execute(Statement::from_sql_and_values(
            backend,
            match backend {
                DbBackend::Postgres => "INSERT INTO module_artifact_rollback_operations (operation_id, installation_id, target_installation_id, expected_revision, actor_id, reason, idempotency_key, target_capability_grant_revision, migration_rollback_mode, source_revision, target_revision, committed_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,NOW())",
                _ => "INSERT INTO module_artifact_rollback_operations (operation_id, installation_id, target_installation_id, expected_revision, actor_id, reason, idempotency_key, target_capability_grant_revision, migration_rollback_mode, source_revision, target_revision, committed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,datetime('now'))",
            }.to_string(),
            vec![
                uuid_value(operation_id, backend),
                uuid_value(request.installation_id, backend),
                uuid_value(target_installation_id, backend),
                expected_revision.into(),
                uuid_value(request.actor_id, backend),
                request.reason.clone().into(),
                uuid_value(request.idempotency_key, backend),
                target_capability_grant_revision.into(),
                request.migration_rollback_mode.as_str().into(),
                source_revision.into(),
                target_revision.into(),
            ],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let source = transaction.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE module_artifact_admissions SET status = 'rolled_back', revision = revision + 1 WHERE installation_id = {} AND revision = {}", placeholders.0, placeholders.1),
            vec![uuid_value(request.installation_id, backend), expected_revision.into()],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if source.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback source is missing or stale".into(),
            ));
        }
        let target = transaction.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE module_artifact_admissions SET status = 'active', revision = revision + 1 WHERE installation_id = {} AND revision = {} AND status IN ('admitted', 'installed', 'inactive', 'rolled_back')", placeholders.0, placeholders.1),
            vec![uuid_value(target_installation_id, backend), target_expected_revision.into()],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if target.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback predecessor is not activatable".into(),
            ));
        }
        transaction.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE module_artifact_installations SET capability_grant_revision = {} WHERE installation_id = {}", placeholders.0, placeholders.1),
            vec![target_capability_grant_revision.into(), uuid_value(target_installation_id, backend)],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let tenant_id = match &request.scope {
            ModuleInstallationScope::Platform => None,
            ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
        };
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    tenant_id,
                    DomainEvent::ModuleArtifactRolledBack {
                        installation_id: request.installation_id,
                        target_installation_id,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleInstallationError::Outbox(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(ArtifactRollbackResult {
            operation_id,
            target_installation_id,
            source_revision: u64::try_from(source_revision).map_err(|_| {
                ModuleInstallationError::Store("rollback source revision is negative".into())
            })?,
            target_revision: u64::try_from(target_revision).map_err(|_| {
                ModuleInstallationError::Store("rollback target revision is negative".into())
            })?,
        })
    }

    /// Replaces admission evidence only when the caller holds the current
    /// revision. Incomplete evidence marks the admission as `failed`;
    /// the immutable CAS blob remains untouched for audit and retention.
    pub async fn reverify_admission(
        &self,
        request: ArtifactAdmissionReverification,
    ) -> Result<u64, ModuleInstallationError> {
        let expected_revision = i64::try_from(request.expected_revision).map_err(|_| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "expected revision exceeds database integer range".into(),
            )
        })?;
        if expected_revision <= 0 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "expected revision must be positive".into(),
            ));
        }
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = transaction.get_database_backend();
        let evidence = serde_json::to_value(&request.evidence)
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let status = if request.evidence.admitted() {
            ArtifactAdmissionStatus::Admitted
        } else {
            ArtifactAdmissionStatus::Failed
        };
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5", "$6"),
            _ => ("?1", "?2", "?3", "?4", "?5", "?6"),
        };
        let (scope_predicate, scope_values) = admission_scope_predicate(&request.scope, backend);
        let sql = format!(
            "UPDATE module_artifact_admissions \
             SET verification_evidence = {}, status = {}, revision = revision + 1 \
             WHERE installation_id = {} AND revision = {} \
               AND payload_digest = {} AND media_type = {}",
            placeholders.0,
            placeholders.1,
            placeholders.2,
            placeholders.3,
            placeholders.4,
            placeholders.5,
        ) + scope_predicate.as_str();
        let mut values = vec![
            SqlValue::Json(Some(Box::new(evidence))),
            status.as_str().into(),
            uuid_value(request.installation_id, backend),
            expected_revision.into(),
            request.evidence.payload_digest.into(),
            request.evidence.media_type.into(),
        ];
        values.extend(scope_values);
        let updated = transaction
            .execute(Statement::from_sql_and_values(backend, sql, values))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if updated.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission is missing, stale, or evidence does not match its immutable CAS identity"
                    .into(),
            ));
        }
        let tenant_id = match &request.scope {
            ModuleInstallationScope::Platform => None,
            ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
        };
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    tenant_id,
                    DomainEvent::ModuleArtifactReverified {
                        installation_id: request.installation_id,
                        status: status.as_str().to_string(),
                        revision: request.expected_revision + 1,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleInstallationError::Outbox(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(request.expected_revision + 1)
    }
}

/// Resolves the one active immutable artifact that may execute a catalog release
/// for a tenant. A tenant-scoped admission shadows an active platform admission;
/// an explicit tenant disable suppresses the platform candidate. This keeps
/// runtime dispatch independent from registry lookups and mutable tags.
#[async_trait]
impl crate::ArtifactInstallationResolver for SeaOrmArtifactInstallationStore {
    async fn resolve(
        &self,
        release: &ArtifactReleaseRef,
        tenant_id: Uuid,
    ) -> Result<InstalledModuleArtifact, String> {
        let transaction = self.db.begin().await.map_err(|error| error.to_string())?;
        configure_rls_scope(&transaction, &ModuleInstallationScope::Tenant { tenant_id })
            .await
            .map_err(|error| error.to_string())?;
        let backend = transaction.get_database_backend();
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4"),
            _ => ("?1", "?2", "?3", "?4"),
        };
        let platform_enabled = match backend {
            DbBackend::Postgres => "COALESCE(tenant_lifecycle.enabled, TRUE) = TRUE",
            _ => "COALESCE(tenant_lifecycle.enabled, 1) = 1",
        };
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT installation.installation_id, installation.scope_kind, installation.tenant_id, \
                     installation.registry, installation.repository, installation.manifest_digest, \
                     installation.slug, installation.version, installation.payload_digest, \
                     CAST(installation.descriptor AS TEXT) AS descriptor, \
                     installation.dependency_graph_revision, installation.dependency_graph_digest, \
                     CAST(installation.dependency_lock AS TEXT) AS dependency_lock, \
                     installation.capability_grant_revision, installation.installed_at \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     LEFT JOIN module_artifact_tenant_lifecycle tenant_lifecycle \
                       ON tenant_lifecycle.installation_id = installation.installation_id \
                      AND tenant_lifecycle.tenant_id = {} \
                     WHERE installation.slug = {} \
                       AND installation.version = {} \
                       AND installation.payload_digest = {} \
                       AND admission.status = 'active' \
                       AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                       WHERE uninstall.installation_id = installation.installation_id) \
                       AND {platform_enabled} \
                       AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {}) \
                            OR (installation.scope_kind = 'platform' \
                                AND installation.tenant_id IS NULL)) \
                     ORDER BY CASE WHEN installation.scope_kind = 'tenant' THEN 0 ELSE 1 END \
                     LIMIT 1",
                    placeholders.3,
                    placeholders.0,
                    placeholders.1,
                    placeholders.2,
                    placeholders.3,
                ),
                vec![
                    release.slug.clone().into(),
                    release.version.clone().into(),
                    release.digest.clone().into(),
                    uuid_value(tenant_id, backend),
                ],
            ))
            .await
            .map_err(|error| error.to_string())?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|error| error.to_string())?;
            return Err("active artifact release is unavailable for the requested tenant".into());
        };

        let installation_id = required_uuid_from_row(&row, "installation_id", backend)
            .map_err(|error| error.to_string())?;
        let scope_kind: String = row
            .try_get("", "scope_kind")
            .map_err(|error| error.to_string())?;
        let scope = match scope_kind.as_str() {
            "platform" => ModuleInstallationScope::Platform,
            "tenant" => ModuleInstallationScope::Tenant {
                tenant_id: required_uuid_from_row(&row, "tenant_id", backend)
                    .map_err(|error| error.to_string())?,
            },
            _ => return Err("artifact installation has an invalid scope".into()),
        };
        let reference = OciArtifactReference {
            registry: row
                .try_get("", "registry")
                .map_err(|error| error.to_string())?,
            repository: row
                .try_get("", "repository")
                .map_err(|error| error.to_string())?,
            digest: row
                .try_get("", "manifest_digest")
                .map_err(|error| error.to_string())?,
        };
        reference.validate().map_err(|error| error.to_string())?;
        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
            &row.try_get::<String>("", "descriptor")
                .map_err(|error| error.to_string())?,
        )
        .map_err(|_| "artifact installation descriptor is invalid".to_string())?;
        descriptor
            .validate()
            .map_err(|_| "artifact installation descriptor is invalid".to_string())?;
        let dependency_lock: ModuleDependencyLockGraph = serde_json::from_str(
            &row.try_get::<String>("", "dependency_lock")
                .map_err(|error| error.to_string())?,
        )
        .map_err(|_| "artifact installation dependency lock is invalid".to_string())?;
        dependency_lock
            .validate()
            .map_err(|_| "artifact installation dependency lock is invalid".to_string())?;
        let persisted_graph_revision: i64 = row
            .try_get("", "dependency_graph_revision")
            .map_err(|error| error.to_string())?;
        let persisted_graph_digest: String = row
            .try_get("", "dependency_graph_digest")
            .map_err(|error| error.to_string())?;
        let capability_grant_revision: i64 = row
            .try_get("", "capability_grant_revision")
            .map_err(|error| error.to_string())?;
        let installed_at = match backend {
            DbBackend::Postgres => row
                .try_get::<DateTime<Utc>>("", "installed_at")
                .map_err(|error| error.to_string())?,
            _ => DateTime::parse_from_rfc3339(
                &row.try_get::<String>("", "installed_at")
                    .map_err(|error| error.to_string())?,
            )
            .map_err(|_| "artifact installation timestamp is invalid".to_string())?
            .with_timezone(&Utc),
        };
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;

        let persisted_slug: String = row.try_get("", "slug").map_err(|error| error.to_string())?;
        let persisted_version: String = row
            .try_get("", "version")
            .map_err(|error| error.to_string())?;
        let persisted_payload_digest: String = row
            .try_get("", "payload_digest")
            .map_err(|error| error.to_string())?;
        if descriptor.slug != persisted_slug
            || descriptor.version != persisted_version
            || descriptor.artifact_digest != persisted_payload_digest
            || descriptor.release_ref() != *release
            || dependency_lock.graph_revision != persisted_graph_revision as u64
            || dependency_lock.graph_digest != persisted_graph_digest
        {
            return Err(
                "artifact installation immutable state does not match its persisted identity"
                    .into(),
            );
        }
        let capability_grant_revision = u64::try_from(capability_grant_revision)
            .map_err(|_| "artifact installation capability revision is invalid".to_string())?;
        Ok(InstalledModuleArtifact {
            installation_id,
            scope,
            reference,
            release: release.clone(),
            descriptor,
            dependency_lock,
            capability_grant_revision,
            installed_at,
        })
    }
}

#[async_trait]
impl ArtifactSandboxPolicyResolver for SeaOrmArtifactSandboxPolicyResolver {
    async fn resolve(
        &self,
        artifact: &InstalledModuleArtifact,
        tenant_id: Uuid,
    ) -> Result<SandboxPolicy, String> {
        if artifact.installation_id.is_nil() || tenant_id.is_nil() {
            return Err("artifact sandbox policy requires non-nil identities".to_string());
        }
        if matches!(&artifact.scope, ModuleInstallationScope::Tenant { tenant_id: scope_tenant } if *scope_tenant != tenant_id)
        {
            return Err("tenant-scoped artifact cannot execute for another tenant".to_string());
        }

        let transaction = self.db.begin().await.map_err(|error| error.to_string())?;
        configure_rls_scope(&transaction, &ModuleInstallationScope::Tenant { tenant_id })
            .await
            .map_err(|error| error.to_string())?;
        let backend = transaction.get_database_backend();
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5"),
            _ => ("?1", "?2", "?3", "?4", "?5"),
        };
        let policy_scope = match &artifact.scope {
            ModuleInstallationScope::Platform => format!(
                "(policy.tenant_id = {} OR policy.tenant_id IS NULL)",
                placeholders.1
            ),
            ModuleInstallationScope::Tenant { .. } => {
                format!("policy.tenant_id = {}", placeholders.1)
            }
        };
        let lifecycle_enabled = match backend {
            DbBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
            _ => "COALESCE(lifecycle.enabled, 1) = 1",
        };
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT policy.capability_grant_revision, CAST(policy.policy AS TEXT) AS policy \
                     FROM module_artifact_sandbox_policies policy \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = policy.installation_id \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     LEFT JOIN module_artifact_tenant_lifecycle lifecycle \
                       ON lifecycle.installation_id = installation.installation_id \
                      AND lifecycle.tenant_id = {tenant} \
                     WHERE policy.installation_id = {installation_id} \
                       AND installation.slug = {slug} \
                       AND installation.version = {version} \
                       AND installation.payload_digest = {digest} \
                       AND admission.status = 'active' \
                       AND (installation.scope_kind = 'platform' \
                            OR (installation.scope_kind = 'tenant' \
                                AND installation.tenant_id = {tenant})) \
                       AND {lifecycle_enabled} \
                       AND {policy_scope} \
                     ORDER BY CASE WHEN policy.tenant_id = {tenant} THEN 0 ELSE 1 END \
                     LIMIT 1",
                    installation_id = placeholders.0,
                    tenant = placeholders.1,
                    slug = placeholders.2,
                    version = placeholders.3,
                    digest = placeholders.4,
                ),
                vec![
                    uuid_value(artifact.installation_id, backend),
                    uuid_value(tenant_id, backend),
                    artifact.release.slug.clone().into(),
                    artifact.release.version.clone().into(),
                    artifact.release.digest.clone().into(),
                ],
            ))
            .await
            .map_err(|error| error.to_string())?;
        transaction
            .commit()
            .await
            .map_err(|error| error.to_string())?;
        let row = row.ok_or_else(|| {
            "no eligible durable sandbox policy exists for the artifact installation".to_string()
        })?;
        let revision: i64 = row
            .try_get("", "capability_grant_revision")
            .map_err(|error| error.to_string())?;
        let revision = u64::try_from(revision)
            .map_err(|_| "sandbox policy revision is outside the supported range".to_string())?;
        if revision != artifact.capability_grant_revision {
            return Err(
                "sandbox policy revision does not match the admitted installation".to_string(),
            );
        }
        let policy: SandboxPolicy = serde_json::from_str(
            &row.try_get::<String>("", "policy")
                .map_err(|error| error.to_string())?,
        )
        .map_err(|_| "sandbox policy is invalid".to_string())?;
        validate_sandbox_policy(artifact, &policy)?;
        Ok(policy)
    }
}

#[async_trait]
impl ArtifactDataMigrationCheckpointStore for SeaOrmArtifactInstallationStore {
    async fn record_data_upgrade_checkpoint(
        &self,
        request: ArtifactMigrationCheckpointRequest,
    ) -> Result<u64, ArtifactDataError> {
        self.record_migration_checkpoint(request)
            .await
            .map_err(|error| ArtifactDataError::MigrationCheckpoint(error.to_string()))
    }
}

fn admission_scope_predicate(
    scope: &ModuleInstallationScope,
    backend: DbBackend,
) -> (String, Vec<SqlValue>) {
    let (scope_kind_placeholder, tenant_placeholder) = match backend {
        DbBackend::Postgres => ("$7", "$8"),
        _ => ("?7", "?8"),
    };
    match scope {
        ModuleInstallationScope::Platform => (
            format!(
                " AND EXISTS (SELECT 1 FROM module_artifact_installations installation \
                 WHERE installation.installation_id = module_artifact_admissions.installation_id \
                 AND installation.scope_kind = {scope_kind_placeholder} \
                 AND installation.tenant_id IS NULL)"
            ),
            vec!["platform".into()],
        ),
        ModuleInstallationScope::Tenant { tenant_id } => (
            format!(
                " AND EXISTS (SELECT 1 FROM module_artifact_installations installation \
                 WHERE installation.installation_id = module_artifact_admissions.installation_id \
                 AND installation.scope_kind = {scope_kind_placeholder} \
                 AND installation.tenant_id = {tenant_placeholder})"
            ),
            vec!["tenant".into(), uuid_value(*tenant_id, backend)],
        ),
    }
}

#[async_trait]
impl ArtifactAdmissionStore for SeaOrmArtifactInstallationStore {
    async fn find_admission(
        &self,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<Option<ArtifactAdmissionResult>, ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &command.scope).await?;
        let backend = transaction.get_database_backend();
        let (scope_kind, scope_tenant_key) = admission_command_scope(command);
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4"),
            _ => ("?1", "?2", "?3", "?4"),
        };
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request_digest, installation_id \
                     FROM module_artifact_admission_commands \
                     WHERE scope_kind = {} AND scope_tenant_key = {} \
                       AND actor_id = {} AND idempotency_key = {}",
                    placeholders.0, placeholders.1, placeholders.2, placeholders.3,
                ),
                vec![
                    scope_kind.into(),
                    scope_tenant_key.into(),
                    uuid_value(command.actor_id, backend),
                    uuid_value(command.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(None);
        };
        let stored_digest: String = row
            .try_get("", "request_digest")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if stored_digest != request_digest {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission idempotency key was already used for a different request".into(),
            ));
        }
        let installation_id = command_installation_id(&row, backend)?.ok_or_else(|| {
            ModuleInstallationError::Store(
                "committed admission command is missing its installation identity".into(),
            )
        })?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(Some(ArtifactAdmissionResult {
            installation_id,
            created: false,
        }))
    }

    async fn commit_admission(
        &self,
        artifact: &InstalledModuleArtifact,
        staged: &StagedArtifactBlob,
        evidence: &ArtifactVerificationEvidence,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &artifact.scope).await?;
        let backend = transaction.get_database_backend();
        let (scope_kind, scope_tenant_key) = admission_command_scope(command);
        let reservation = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                admission_command_insert_sql(backend),
                vec![
                    scope_kind.into(),
                    scope_tenant_key.clone().into(),
                    uuid_value(command.actor_id, backend),
                    uuid_value(command.idempotency_key, backend),
                    request_digest.into(),
                    now_value(backend),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if reservation.rows_affected() != 1 {
            let existing = existing_admission_command(
                &transaction,
                backend,
                scope_kind,
                &scope_tenant_key,
                command.actor_id,
                command.idempotency_key,
            )
            .await?
            .ok_or_else(|| {
                ModuleInstallationError::Store(
                    "admission command reservation disappeared after a conflict".into(),
                )
            })?;
            if existing.0 != request_digest {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "admission idempotency key was already used for a different request".into(),
                ));
            }
            let installation_id = existing.1.ok_or_else(|| {
                ModuleInstallationError::Store(
                    "admission command is still incomplete after its transaction committed".into(),
                )
            })?;
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(ArtifactAdmissionResult {
                installation_id,
                created: false,
            });
        }
        let previous_installation_id =
            previous_installation_id(&transaction, artifact, backend).await?;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                installation_insert_sql(backend),
                installation_values(artifact, previous_installation_id, backend)?,
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                sandbox_policy_insert_sql(backend),
                sandbox_policy_values(artifact, &command.sandbox_policy, backend)?,
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let bound = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                admission_command_bind_sql(backend),
                vec![
                    uuid_value(artifact.installation_id, backend),
                    scope_kind.into(),
                    scope_tenant_key.into(),
                    uuid_value(command.actor_id, backend),
                    uuid_value(command.idempotency_key, backend),
                    request_digest.into(),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if bound.rows_affected() != 1 {
            return Err(ModuleInstallationError::Store(
                "admission command reservation became unavailable before installation binding"
                    .into(),
            ));
        }
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                admission_insert_sql(backend),
                admission_values(artifact, staged, evidence, backend)?,
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let tenant_id = match &artifact.scope {
            ModuleInstallationScope::Platform => None,
            ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
        };
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    tenant_id,
                    DomainEvent::ModuleArtifactAdmitted {
                        installation_id: artifact.installation_id,
                        artifact_digest: artifact.descriptor.artifact_digest.clone(),
                        media_type: staged.media_type.clone(),
                        size_bytes: staged.size_bytes,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleInstallationError::Outbox(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(ArtifactAdmissionResult {
            installation_id: artifact.installation_id,
            created: true,
        })
    }

    async fn unfinished_admissions(
        &self,
    ) -> Result<Vec<ArtifactAdmissionRecoveryRecord>, ModuleInstallationError> {
        Ok(Vec::new())
    }

    async fn referenced_blob_digests(&self) -> Result<BTreeSet<String>, ModuleInstallationError> {
        let backend = self.db.get_database_backend();
        let rows = self
            .db
            .query_all(Statement::from_string(
                backend,
                "SELECT admission.payload_digest FROM module_artifact_admissions admission \
                 WHERE NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                 WHERE uninstall.installation_id = admission.installation_id)"
                    .to_string(),
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        rows.into_iter()
            .map(|row| {
                row.try_get("", "payload_digest")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))
            })
            .collect()
    }
}

fn admission_command_scope(command: &ArtifactAdmissionCommand) -> (&'static str, String) {
    match &command.scope {
        ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
    }
}

fn now_value(backend: DbBackend) -> SqlValue {
    let now = Utc::now();
    match backend {
        DbBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(Box::new(now))),
        _ => now.to_rfc3339().into(),
    }
}

fn admission_command_insert_sql(backend: DbBackend) -> String {
    let placeholders = match backend {
        DbBackend::Postgres => (1..=6).map(|index| format!("${index}")).collect::<Vec<_>>(),
        _ => (1..=6).map(|index| format!("?{index}")).collect::<Vec<_>>(),
    };
    format!(
        "INSERT INTO module_artifact_admission_commands (\
            scope_kind, scope_tenant_key, actor_id, idempotency_key, request_digest, committed_at\
         ) VALUES ({}) ON CONFLICT DO NOTHING",
        placeholders.join(", "),
    )
}

fn admission_command_bind_sql(backend: DbBackend) -> String {
    let placeholders = match backend {
        DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5", "$6"),
        _ => ("?1", "?2", "?3", "?4", "?5", "?6"),
    };
    format!(
        "UPDATE module_artifact_admission_commands SET installation_id = {} \
         WHERE scope_kind = {} AND scope_tenant_key = {} AND actor_id = {} \
           AND idempotency_key = {} AND request_digest = {} AND installation_id IS NULL",
        placeholders.0,
        placeholders.1,
        placeholders.2,
        placeholders.3,
        placeholders.4,
        placeholders.5,
    )
}

async fn existing_admission_command<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    scope_kind: &str,
    scope_tenant_key: &str,
    actor_id: Uuid,
    idempotency_key: Uuid,
) -> Result<Option<(String, Option<Uuid>)>, ModuleInstallationError> {
    let placeholders = match backend {
        DbBackend::Postgres => ("$1", "$2", "$3", "$4"),
        _ => ("?1", "?2", "?3", "?4"),
    };
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_digest, installation_id \
                 FROM module_artifact_admission_commands \
                 WHERE scope_kind = {} AND scope_tenant_key = {} \
                   AND actor_id = {} AND idempotency_key = {}",
                placeholders.0, placeholders.1, placeholders.2, placeholders.3,
            ),
            vec![
                scope_kind.into(),
                scope_tenant_key.into(),
                uuid_value(actor_id, backend),
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    row.map(|row| {
        let request_digest = row
            .try_get("", "request_digest")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok((request_digest, command_installation_id(&row, backend)?))
    })
    .transpose()
}

fn command_installation_id(
    row: &sea_orm::QueryResult,
    backend: DbBackend,
) -> Result<Option<Uuid>, ModuleInstallationError> {
    match backend {
        DbBackend::Postgres => row
            .try_get::<Option<Uuid>>("", "installation_id")
            .map_err(|error| ModuleInstallationError::Store(error.to_string())),
        _ => row
            .try_get::<Option<String>>("", "installation_id")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .map(|value| {
                value
                    .parse::<Uuid>()
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))
            })
            .transpose(),
    }
}

fn required_uuid_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ModuleInstallationError> {
    match backend {
        DbBackend::Postgres => row
            .try_get::<Uuid>("", column)
            .map_err(|error| ModuleInstallationError::Store(error.to_string())),
        _ => row
            .try_get::<String>("", column)
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .parse::<Uuid>()
            .map_err(|error| ModuleInstallationError::Store(error.to_string())),
    }
}

async fn configure_rls_scope<C: ConnectionTrait>(
    connection: &C,
    scope: &ModuleInstallationScope,
) -> Result<(), ModuleInstallationError> {
    if let (DbBackend::Postgres, ModuleInstallationScope::Tenant { tenant_id }) =
        (connection.get_database_backend(), scope)
    {
        connection
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('rustok.tenant_id', $1, true)",
                vec![tenant_id.to_string().into()],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    }
    Ok(())
}

fn installation_insert_sql(backend: DbBackend) -> String {
    let placeholders = match backend {
        DbBackend::Postgres => (1..=19)
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>(),
        _ => (1..=19)
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>(),
    };
    format!(
        "INSERT INTO module_artifact_installations (\
            installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
            slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
            dependency_graph_revision, dependency_graph_digest, dependency_lock, installed_at, \
            previous_installation_id, capability_grant_revision\
         ) VALUES ({})",
        placeholders.join(", ")
    )
}

fn sandbox_policy_insert_sql(backend: DbBackend) -> String {
    let placeholders = match backend {
        DbBackend::Postgres => (1..=5).map(|index| format!("${index}")).collect::<Vec<_>>(),
        _ => (1..=5).map(|index| format!("?{index}")).collect::<Vec<_>>(),
    };
    format!(
        "INSERT INTO module_artifact_sandbox_policies \
         (installation_id, tenant_id, capability_grant_revision, policy, created_at) \
         VALUES ({})",
        placeholders.join(", ")
    )
}

fn sandbox_policy_values(
    artifact: &InstalledModuleArtifact,
    policy: &SandboxPolicy,
    backend: DbBackend,
) -> Result<Vec<SqlValue>, ModuleInstallationError> {
    let tenant_id = match &artifact.scope {
        ModuleInstallationScope::Platform => None,
        ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
    };
    let revision = i64::try_from(artifact.capability_grant_revision).map_err(|_| {
        ModuleInstallationError::AdmissionRevisionConflict(
            "sandbox policy revision exceeds database range".to_string(),
        )
    })?;
    let policy = serde_json::to_value(policy)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(vec![
        uuid_value(artifact.installation_id, backend),
        optional_uuid_value(tenant_id, backend),
        revision.into(),
        SqlValue::Json(Some(Box::new(policy))),
        now_value(backend),
    ])
}

fn validate_sandbox_policy(
    artifact: &InstalledModuleArtifact,
    policy: &SandboxPolicy,
) -> Result<(), String> {
    validate_sandbox_policy_for_admission(&artifact.descriptor, policy)
        .map_err(|error| error.to_string())
}

fn validate_sandbox_policy_for_admission(
    descriptor: &ModuleArtifactDescriptor,
    policy: &SandboxPolicy,
) -> Result<(), ModuleInstallationError> {
    let limits = policy.limits;
    if limits.wall_clock_ms == 0
        || limits.instruction_budget == 0
        || limits.max_memory_bytes == 0
        || limits.max_output_bytes == 0
        || limits.max_concurrency == 0
        || limits.max_capability_calls == 0
        || limits.max_capability_input_bytes == 0
        || limits.max_capability_calls_per_second == 0
    {
        return Err(ModuleInstallationError::InvalidSandboxPolicy(
            "sandbox policy limits must be positive".to_string(),
        ));
    }
    let mut granted = HashSet::new();
    for grant in &policy.grants {
        let name = grant.name.as_str();
        if !descriptor
            .capabilities
            .iter()
            .any(|declared| declared == &grant.name)
        {
            return Err(ModuleInstallationError::UndeclaredCapability(
                grant.name.as_str().to_string(),
            ));
        }
        if !granted.insert(name) {
            return Err(ModuleInstallationError::InvalidSandboxPolicy(
                "sandbox policy contains a duplicate capability grant".to_string(),
            ));
        }
    }
    Ok(())
}

fn installation_values(
    artifact: &InstalledModuleArtifact,
    previous_installation_id: Option<Uuid>,
    backend: DbBackend,
) -> Result<Vec<SqlValue>, ModuleInstallationError> {
    let (scope_kind, tenant_id) = match &artifact.scope {
        ModuleInstallationScope::Platform => ("platform", None),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
    };
    let descriptor = serde_json::to_value(&artifact.descriptor)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let dependency_lock = serde_json::to_value(&artifact.dependency_lock)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let dependency_graph_revision = i64::try_from(artifact.dependency_lock.graph_revision)
        .map_err(|_| {
            ModuleInstallationError::DependencyLock(
                "graph revision exceeds database integer range".into(),
            )
        })?;
    let capability_grant_revision =
        i64::try_from(artifact.capability_grant_revision).map_err(|_| {
            ModuleInstallationError::DependencyLock(
                "capability grant revision exceeds database integer range".into(),
            )
        })?;
    let installation_id = uuid_value(artifact.installation_id, backend);
    let tenant_id = optional_uuid_value(tenant_id, backend);
    let installed_at = match backend {
        DbBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(Box::new(artifact.installed_at))),
        _ => artifact.installed_at.to_rfc3339().into(),
    };
    Ok(vec![
        installation_id,
        scope_kind.into(),
        tenant_id,
        artifact.reference.registry.clone().into(),
        artifact.reference.repository.clone().into(),
        artifact.reference.digest.clone().into(),
        artifact.release.slug.clone().into(),
        artifact.release.version.clone().into(),
        artifact.descriptor.payload_kind.as_str().into(),
        artifact.descriptor.runtime_abi.clone().into(),
        artifact.descriptor.artifact_digest.clone().into(),
        artifact.descriptor.entrypoint.clone().into(),
        SqlValue::Json(Some(Box::new(descriptor))),
        dependency_graph_revision.into(),
        artifact.dependency_lock.graph_digest.clone().into(),
        SqlValue::Json(Some(Box::new(dependency_lock))),
        installed_at,
        optional_uuid_value(previous_installation_id, backend),
        capability_grant_revision.into(),
    ])
}

async fn previous_installation_id<C: ConnectionTrait>(
    connection: &C,
    artifact: &InstalledModuleArtifact,
    backend: DbBackend,
) -> Result<Option<Uuid>, ModuleInstallationError> {
    let (scope_kind, tenant_id) = match &artifact.scope {
        ModuleInstallationScope::Platform => ("platform", None),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
    };
    let placeholders = match backend {
        DbBackend::Postgres => ("$1", "$2", "$3", "$4"),
        _ => ("?1", "?2", "?3", "?4"),
    };
    let sql = format!(
        "SELECT installation_id FROM module_artifact_installations \
         WHERE scope_kind = {} \
           AND ((tenant_id IS NULL AND {} IS NULL) OR tenant_id = {}) \
           AND slug = {} \
         ORDER BY installed_at DESC, installation_id DESC LIMIT 1",
        placeholders.0, placeholders.1, placeholders.2, placeholders.3,
    );
    let values = vec![
        scope_kind.into(),
        optional_uuid_value(tenant_id, backend),
        optional_uuid_value(tenant_id, backend),
        artifact.release.slug.clone().into(),
    ];
    let row = connection
        .query_one(Statement::from_sql_and_values(backend, sql, values))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    row.map(|row| match backend {
        DbBackend::Postgres => row
            .try_get("", "installation_id")
            .map_err(|error| ModuleInstallationError::Store(error.to_string())),
        _ => row
            .try_get::<String>("", "installation_id")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .parse::<Uuid>()
            .map_err(|error| ModuleInstallationError::Store(error.to_string())),
    })
    .transpose()
}

fn admission_insert_sql(backend: DbBackend) -> String {
    let placeholders = match backend {
        DbBackend::Postgres => (1..=9).map(|index| format!("${index}")).collect::<Vec<_>>(),
        _ => (1..=9).map(|index| format!("?{index}")).collect::<Vec<_>>(),
    };
    format!(
        "INSERT INTO module_artifact_admissions (\
            stage_id, installation_id, payload_digest, media_type, size_bytes, verification_evidence, status, revision, committed_at\
         ) VALUES ({})",
        placeholders.join(", ")
    )
}

fn admission_values(
    artifact: &InstalledModuleArtifact,
    staged: &StagedArtifactBlob,
    evidence: &ArtifactVerificationEvidence,
    backend: DbBackend,
) -> Result<Vec<SqlValue>, ModuleInstallationError> {
    let committed_at = Utc::now();
    if evidence.manifest_digest != artifact.reference.digest
        || evidence.payload_digest != staged.digest
        || evidence.media_type != staged.media_type
    {
        return Err(ModuleInstallationError::TrustVerification(
            "verification evidence does not match the admitted artifact".into(),
        ));
    }
    let evidence = serde_json::to_value(evidence)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let committed_at = match backend {
        DbBackend::Postgres => SqlValue::ChronoDateTimeUtc(Some(Box::new(committed_at))),
        _ => committed_at.to_rfc3339().into(),
    };
    let size_bytes = i64::try_from(staged.size_bytes).map_err(|_| {
        ModuleInstallationError::Blob("artifact payload exceeds database size range".into())
    })?;
    Ok(vec![
        uuid_value(staged.stage_id, backend),
        uuid_value(artifact.installation_id, backend),
        staged.digest.clone().into(),
        staged.media_type.clone().into(),
        size_bytes.into(),
        SqlValue::Json(Some(Box::new(evidence))),
        ArtifactAdmissionStatus::Admitted.as_str().into(),
        1_i64.into(),
        committed_at,
    ])
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::Uuid(Some(Box::new(value))),
        _ => value.to_string().into(),
    }
}

fn optional_uuid_value(value: Option<Uuid>, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::Uuid(value.map(Box::new)),
        _ => value.map(|value| value.to_string()).into(),
    }
}

/// Coordinates immutable OCI resolution and durable admission. The concrete OCI
/// client and database adapter are host infrastructure; this module owns the
/// identity, validation and lifecycle boundary.
pub struct ModuleInstaller<R, S, B, P> {
    registry: R,
    admission: ArtifactAdmissionService<S, B>,
    verifier: Arc<dyn TrustVerifier>,
    permission_registrar: P,
    trust_policy: TrustPolicyRevision,
    limits: ArtifactAdmissionLimits,
}

/// Owner-owned admission entrypoint. Infrastructure supplies the durable CAS
/// and transactional metadata/outbox adapters; this service owns their order.
pub struct ArtifactAdmissionService<S, B> {
    store: S,
    blobs: B,
}

impl<S, B> ArtifactAdmissionService<S, B>
where
    S: ArtifactAdmissionStore,
    B: DurableArtifactBlobStore,
{
    pub fn new(store: S, blobs: B) -> Self {
        Self { store, blobs }
    }

    pub async fn find_admission(
        &self,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<Option<ArtifactAdmissionResult>, ModuleInstallationError> {
        self.store.find_admission(command, request_digest).await
    }

    pub async fn admit(
        &self,
        artifact: &InstalledModuleArtifact,
        media_type: &str,
        payload: &[u8],
        evidence: &ArtifactVerificationEvidence,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
        artifact.validate_dependency_lock()?;
        let staged = self
            .blobs
            .stage(&artifact.descriptor.artifact_digest, media_type, payload)
            .await?;
        if let Err(error) = self.blobs.publish(&staged).await {
            let _ = self.blobs.discard(&staged).await;
            return Err(error);
        }
        self.store
            .commit_admission(artifact, &staged, evidence, command, request_digest)
            .await
    }

    pub async fn admit_file(
        &self,
        artifact: &InstalledModuleArtifact,
        media_type: &str,
        source: &std::path::Path,
        evidence: &ArtifactVerificationEvidence,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
        artifact.validate_dependency_lock()?;
        let staged = self
            .blobs
            .stage_file(&artifact.descriptor.artifact_digest, media_type, source)
            .await?;
        if let Err(error) = self.blobs.publish(&staged).await {
            let _ = self.blobs.discard(&staged).await;
            return Err(error);
        }
        self.store
            .commit_admission(artifact, &staged, evidence, command, request_digest)
            .await
    }
}

impl<R, S, B, P> ModuleInstaller<R, S, B, P>
where
    R: ArtifactRegistry,
    S: ArtifactAdmissionStore,
    B: DurableArtifactBlobStore,
    P: ArtifactPermissionRegistrationPort,
{
    pub fn new(
        registry: R,
        store: S,
        blobs: B,
        verifier: Arc<dyn TrustVerifier>,
        trust_policy: TrustPolicyRevision,
        permission_registrar: P,
    ) -> Self {
        Self {
            registry,
            admission: ArtifactAdmissionService::new(store, blobs),
            verifier,
            permission_registrar,
            trust_policy,
            limits: ArtifactAdmissionLimits::default(),
        }
    }

    pub fn with_admission_limits(mut self, limits: ArtifactAdmissionLimits) -> Self {
        self.limits = limits;
        self
    }

    pub async fn admit(
        &self,
        command: ArtifactAdmissionCommand,
    ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
        command.validate()?;
        let request_digest = command.request_digest()?;
        let existing = self
            .admission
            .find_admission(&command, &request_digest)
            .await?;
        let reference = command.reference.clone();
        let package = self.registry.fetch(&reference, self.limits).await?;
        if package.reference != reference {
            return Err(ModuleInstallationError::RegistryIdentityMismatch {
                requested: reference.canonical(),
                received: package.reference.canonical(),
            });
        }
        package.verify(self.limits).await?;
        if let Some(existing) = existing {
            self.register_admitted_permissions(
                existing.installation_id,
                &command.scope,
                &package.descriptor,
            )
            .await?;
            return Ok(existing);
        }
        if package.descriptor.payload_kind == ArtifactPayloadKind::StaticPromoted {
            return Err(ModuleInstallationError::StaticPromotionRequired);
        }
        validate_sandbox_policy_for_admission(&package.descriptor, &command.sandbox_policy)?;
        let verification_request = TrustVerificationRequest {
            reference: package.reference.clone(),
            descriptor: package.descriptor.clone(),
            trust_policy_revision: self.trust_policy.trust_policy_revision,
            capability_policy_revision: self.trust_policy.capability_policy_revision,
        };
        let decision = self
            .verifier
            .verify(verification_request.clone())
            .await
            .map_err(ModuleInstallationError::TrustVerification)?;
        if decision.trust_policy_revision != verification_request.trust_policy_revision
            || decision.capability_policy_revision
                != verification_request.capability_policy_revision
            || !decision.admitted()
        {
            return Err(ModuleInstallationError::TrustVerification(
                "verification decision is not admitted for the requested policy revisions".into(),
            ));
        }
        let release = package.release_ref();
        let installed_at = Utc::now();
        let artifact = InstalledModuleArtifact {
            installation_id: Uuid::new_v4(),
            scope: command.scope.clone(),
            reference: package.reference,
            release,
            descriptor: package.descriptor,
            dependency_lock: command.dependency_lock.clone(),
            capability_grant_revision: self.trust_policy.capability_grant_revision,
            installed_at,
        };
        let evidence = ArtifactVerificationEvidence::from_decision(
            &artifact,
            &package.media_type,
            decision,
            installed_at,
        );
        let result = match package.payload {
            ArtifactPayloadSource::Bytes(payload) => {
                self.admission
                    .admit(
                        &artifact,
                        &package.media_type,
                        &payload,
                        &evidence,
                        &command,
                        &request_digest,
                    )
                    .await?
            }
            ArtifactPayloadSource::TemporaryFile(path) => {
                let result = self
                    .admission
                    .admit_file(
                        &artifact,
                        &package.media_type,
                        &path,
                        &evidence,
                        &command,
                        &request_digest,
                    )
                    .await;
                let _ = tokio::fs::remove_file(&path).await;
                result?
            }
        };
        self.register_admitted_permissions(
            result.installation_id,
            &command.scope,
            &artifact.descriptor,
        )
        .await?;
        Ok(result)
    }

    async fn register_admitted_permissions(
        &self,
        installation_id: Uuid,
        scope: &ModuleInstallationScope,
        descriptor: &ModuleArtifactDescriptor,
    ) -> Result<(), ModuleInstallationError> {
        if descriptor.permissions.is_empty() {
            return Ok(());
        }
        let scope = match scope {
            ModuleInstallationScope::Platform => ArtifactPermissionScope::Platform,
            ModuleInstallationScope::Tenant { tenant_id } => ArtifactPermissionScope::Tenant {
                tenant_id: *tenant_id,
            },
        };
        self.permission_registrar
            .register_admitted_permissions(ArtifactPermissionRegistrationRequest {
                installation_id,
                scope,
                module_slug: descriptor.slug.clone(),
                release_digest: descriptor.artifact_digest.clone(),
                permissions: descriptor
                    .permissions
                    .iter()
                    .map(|permission| ArtifactPermissionRegistration {
                        key: permission.key.clone(),
                        localizations: permission.localizations.clone(),
                    })
                    .collect(),
            })
            .await
            .map_err(|error| {
                ModuleInstallationError::PermissionRegistration(format!(
                    "{}: {}",
                    error.code, error.message
                ))
            })
    }
}

#[derive(Debug, Error)]
pub enum ModuleInstallationError {
    #[error(transparent)]
    Artifact(#[from] ModuleArtifactError),
    #[error("invalid OCI artifact reference: {0}")]
    InvalidOciReference(String),
    #[error("artifact payload digest mismatch: expected `{expected}`, received `{actual}")]
    PayloadDigestMismatch { expected: String, actual: String },
    #[error("artifact {kind} size `{actual}` exceeds admission limit `{limit}`")]
    ArtifactTooLarge {
        kind: &'static str,
        limit: u64,
        actual: u64,
    },
    #[error("artifact media type must be `{expected}`, received `{actual}")]
    UnexpectedMediaType { expected: String, actual: String },
    #[error("registry returned `{received}` for requested artifact `{requested}")]
    RegistryIdentityMismatch { requested: String, received: String },
    #[error("static promotion is a build-time distribution path and cannot be runtime-installed")]
    StaticPromotionRequired,
    #[error("sandbox policy grants undeclared artifact capability `{0}")]
    UndeclaredCapability(String),
    #[error("artifact sandbox policy is invalid: {0}")]
    InvalidSandboxPolicy(String),
    #[error("artifact Rhai binding serialization failed: {0}")]
    RhaiBinding(String),
    #[error("artifact registry error: {0}")]
    Registry(String),
    #[error("artifact installation store error: {0}")]
    Store(String),
    #[error("artifact blob store error: {0}")]
    Blob(String),
    #[error("artifact admission outbox error: {0}")]
    Outbox(String),
    #[error("artifact admission revision conflict: {0}")]
    AdmissionRevisionConflict(String),
    #[error("admitted artifact blob `{0}` is unavailable")]
    BlobNotFound(String),
    #[error("artifact dependency lock is invalid: {0}")]
    DependencyLock(String),
    #[error("artifact trust verification failed: {0}")]
    TrustVerification(String),
    #[error("artifact permission registration failed: {0}")]
    PermissionRegistration(String),
}

fn media_type_for(kind: ArtifactPayloadKind) -> &'static str {
    match kind {
        ArtifactPayloadKind::Rhai => RHAI_MEDIA_TYPE,
        ArtifactPayloadKind::WasmComponent => WASM_COMPONENT_MEDIA_TYPE,
        ArtifactPayloadKind::Sidecar => SIDECAR_MEDIA_TYPE,
        ArtifactPayloadKind::StaticPromoted => STATIC_PROMOTION_MEDIA_TYPE,
    }
}

fn valid_repository_segment(value: &str) -> bool {
    value.chars().all(|character| {
        character.is_ascii_lowercase()
            || character.is_ascii_digit()
            || matches!(character, '-' | '_' | '.')
    })
}

pub(crate) fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

pub(crate) fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn validate_migration_checkpoint_request(
    request: &ArtifactMigrationCheckpointRequest,
) -> Result<(), ModuleInstallationError> {
    if request.expected_revision == 0 || !request.checkpoint.is_object() {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "migration checkpoint requires a positive revision and an object value".into(),
        ));
    }
    let checkpoint_size = serde_json::to_vec(&request.checkpoint)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
        .len();
    if checkpoint_size > MAX_ARTIFACT_MIGRATION_CHECKPOINT_BYTES {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(format!(
            "migration checkpoint exceeds the {} byte owner metadata limit",
            MAX_ARTIFACT_MIGRATION_CHECKPOINT_BYTES
        )));
    }
    Ok(())
}

fn validate_lifecycle_command(
    installation_id: Uuid,
    scope: &ModuleInstallationScope,
    expected_revision: u64,
    actor_id: Uuid,
    reason: &str,
    idempotency_key: Uuid,
) -> Result<(), ModuleInstallationError> {
    let valid_scope = matches!(scope, ModuleInstallationScope::Platform)
        || matches!(scope, ModuleInstallationScope::Tenant { tenant_id } if !tenant_id.is_nil());
    if installation_id.is_nil()
        || !valid_scope
        || expected_revision == 0
        || actor_id.is_nil()
        || reason.trim().is_empty()
        || idempotency_key.is_nil()
    {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "lifecycle command requires non-nil identities, a valid scope, a positive revision, and a non-empty reason".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::Utc;
    use rustok_core::MigrationSource;
    use rustok_sandbox::{CapabilityGrant, CapabilityName, ExecutionPhase, SandboxPolicy};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use serde_json::json;

    use super::*;
    use crate::ArtifactModuleKind;

    #[test]
    fn migration_checkpoint_rejects_oversized_owner_metadata() {
        let request = ArtifactMigrationCheckpointRequest {
            installation_id: Uuid::new_v4(),
            scope: ModuleInstallationScope::Platform,
            expected_revision: 1,
            checkpoint: json!({ "payload": "x".repeat(MAX_ARTIFACT_MIGRATION_CHECKPOINT_BYTES) }),
            has_irreversible_migration: false,
        };

        assert!(matches!(
            validate_migration_checkpoint_request(&request),
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
    }

    #[test]
    fn lifecycle_command_requires_non_nil_identities_and_tenant_scope() {
        let installation_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let idempotency_key = Uuid::new_v4();
        let valid = || {
            validate_lifecycle_command(
                installation_id,
                &ModuleInstallationScope::Tenant {
                    tenant_id: Uuid::new_v4(),
                },
                1,
                actor_id,
                "operator request",
                idempotency_key,
            )
        };
        assert!(valid().is_ok());
        assert!(validate_lifecycle_command(
            Uuid::nil(),
            &ModuleInstallationScope::Platform,
            1,
            actor_id,
            "operator request",
            idempotency_key,
        )
        .is_err());
        assert!(validate_lifecycle_command(
            installation_id,
            &ModuleInstallationScope::Tenant {
                tenant_id: Uuid::nil(),
            },
            1,
            actor_id,
            "operator request",
            idempotency_key,
        )
        .is_err());
        assert!(validate_lifecycle_command(
            installation_id,
            &ModuleInstallationScope::Platform,
            1,
            Uuid::nil(),
            "operator request",
            idempotency_key,
        )
        .is_err());
        assert!(validate_lifecycle_command(
            installation_id,
            &ModuleInstallationScope::Platform,
            1,
            actor_id,
            "operator request",
            Uuid::nil(),
        )
        .is_err());
    }

    struct FixtureRegistry(ModuleArtifactPackage);

    struct AllowTrustVerifier;

    #[async_trait]
    impl TrustVerifier for AllowTrustVerifier {
        async fn verify(
            &self,
            request: TrustVerificationRequest,
        ) -> Result<TrustVerificationDecision, String> {
            Ok(TrustVerificationDecision {
                signer_identity: "test.signer.example".to_string(),
                trust_policy_revision: request.trust_policy_revision,
                capability_policy_revision: request.capability_policy_revision,
                signature_verified: true,
                provenance_verified: true,
                sbom_verified: true,
                evidence_references: vec!["test://verification/evidence".to_string()],
            })
        }
    }

    fn trust_verifier() -> Arc<dyn TrustVerifier> {
        Arc::new(AllowTrustVerifier)
    }

    fn trust_policy() -> TrustPolicyRevision {
        TrustPolicyRevision {
            trust_policy_revision: 1,
            capability_policy_revision: 1,
            capability_grant_revision: 1,
        }
    }

    struct AllowArtifactPermissionRegistrar;

    #[async_trait]
    impl ArtifactPermissionRegistrationPort for AllowArtifactPermissionRegistrar {
        async fn register_admitted_permissions(
            &self,
            _request: ArtifactPermissionRegistrationRequest,
        ) -> Result<(), rustok_api::PortError> {
            Ok(())
        }
    }

    #[derive(Clone, Default)]
    struct RecordingArtifactPermissionRegistrar(
        Arc<Mutex<Vec<ArtifactPermissionRegistrationRequest>>>,
    );

    #[async_trait]
    impl ArtifactPermissionRegistrationPort for RecordingArtifactPermissionRegistrar {
        async fn register_admitted_permissions(
            &self,
            request: ArtifactPermissionRegistrationRequest,
        ) -> Result<(), rustok_api::PortError> {
            self.0.lock().expect("registrar lock").push(request);
            Ok(())
        }
    }

    #[async_trait]
    impl ArtifactRegistry for FixtureRegistry {
        async fn fetch(
            &self,
            _reference: &OciArtifactReference,
            _limits: ArtifactAdmissionLimits,
        ) -> Result<ModuleArtifactPackage, ModuleInstallationError> {
            Ok(self.0.clone())
        }
    }

    #[derive(Clone, Default)]
    struct CapturingStore(Arc<Mutex<Vec<InstalledModuleArtifact>>>);

    #[async_trait]
    impl ArtifactAdmissionStore for CapturingStore {
        async fn find_admission(
            &self,
            _command: &ArtifactAdmissionCommand,
            _request_digest: &str,
        ) -> Result<Option<ArtifactAdmissionResult>, ModuleInstallationError> {
            Ok(None)
        }

        async fn commit_admission(
            &self,
            artifact: &InstalledModuleArtifact,
            _staged: &StagedArtifactBlob,
            _evidence: &ArtifactVerificationEvidence,
            _command: &ArtifactAdmissionCommand,
            _request_digest: &str,
        ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
            self.0.lock().expect("store lock").push(artifact.clone());
            Ok(ArtifactAdmissionResult {
                installation_id: artifact.installation_id,
                created: true,
            })
        }

        async fn unfinished_admissions(
            &self,
        ) -> Result<Vec<ArtifactAdmissionRecoveryRecord>, ModuleInstallationError> {
            Ok(Vec::new())
        }

        async fn referenced_blob_digests(
            &self,
        ) -> Result<BTreeSet<String>, ModuleInstallationError> {
            Ok(self
                .0
                .lock()
                .expect("store lock")
                .iter()
                .map(|artifact| artifact.descriptor.artifact_digest.clone())
                .collect())
        }
    }

    struct RecoveryStore(Vec<ArtifactAdmissionRecoveryRecord>);

    struct AllowBlobDeletion;

    #[async_trait]
    impl ArtifactBlobRetentionPolicy for AllowBlobDeletion {
        async fn may_delete(&self, _digest: &str) -> Result<bool, ModuleInstallationError> {
            Ok(true)
        }
    }

    #[async_trait]
    impl ArtifactAdmissionStore for RecoveryStore {
        async fn find_admission(
            &self,
            _command: &ArtifactAdmissionCommand,
            _request_digest: &str,
        ) -> Result<Option<ArtifactAdmissionResult>, ModuleInstallationError> {
            Ok(None)
        }

        async fn commit_admission(
            &self,
            artifact: &InstalledModuleArtifact,
            _staged: &StagedArtifactBlob,
            _evidence: &ArtifactVerificationEvidence,
            _command: &ArtifactAdmissionCommand,
            _request_digest: &str,
        ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
            Ok(ArtifactAdmissionResult {
                installation_id: artifact.installation_id,
                created: true,
            })
        }

        async fn unfinished_admissions(
            &self,
        ) -> Result<Vec<ArtifactAdmissionRecoveryRecord>, ModuleInstallationError> {
            Ok(self.0.clone())
        }

        async fn referenced_blob_digests(
            &self,
        ) -> Result<BTreeSet<String>, ModuleInstallationError> {
            Ok(BTreeSet::new())
        }
    }

    fn package(kind: ArtifactPayloadKind) -> ModuleArtifactPackage {
        let payload = b"let result = input.value; result".to_vec();
        let digest = sha256_digest(&payload);
        ModuleArtifactPackage {
            reference: OciArtifactReference {
                registry: "registry.example".to_string(),
                repository: "modules/sample_module".to_string(),
                digest: format!("sha256:{}", "a".repeat(64)),
            },
            descriptor: ModuleArtifactDescriptor {
                schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                payload_kind: kind,
                module_kind: ArtifactModuleKind::Optional,
                runtime_abi: "rustok:module/runtime@1".to_string(),
                platform_compatibility: "^0.1".to_string(),
                required_features: Vec::new(),
                artifact_digest: digest,
                entrypoint: "main".to_string(),
                capabilities: vec![CapabilityName::new("platform.events").expect("capability")],
                bindings: Vec::new(),
                dependencies: Vec::new(),
                permissions: Vec::new(),
                schema_documents: Vec::new(),
                settings_schema_digest: None,
                data_schema_digest: None,
                ui_contributions: Vec::new(),
                persistence_contract: None,
            },
            media_type: media_type_for(kind).to_string(),
            payload: ArtifactPayloadSource::Bytes(payload),
        }
    }

    fn empty_dependency_lock() -> ModuleDependencyLockGraph {
        ModuleDependencyLockGraph::create(0, Vec::new()).expect("empty dependency lock")
    }

    fn admission_command(
        reference: OciArtifactReference,
        scope: ModuleInstallationScope,
    ) -> ArtifactAdmissionCommand {
        ArtifactAdmissionCommand {
            reference,
            scope,
            dependency_lock: empty_dependency_lock(),
            sandbox_policy: SandboxPolicy::default(),
            actor_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        }
    }

    #[tokio::test]
    async fn reconciler_discards_only_unpublished_staging() {
        let blobs = InMemoryArtifactBlobStore::default();
        let payload = b"staged module payload";
        let digest = sha256_digest(payload);
        let staged = blobs
            .stage(&digest, RHAI_MEDIA_TYPE, payload)
            .await
            .expect("stage blob");
        let reconciler = ArtifactAdmissionReconciler::new(
            blobs,
            RecoveryStore(vec![ArtifactAdmissionRecoveryRecord {
                staged,
                stage: ArtifactAdmissionStage::Staged,
            }]),
        );

        assert_eq!(
            reconciler
                .discard_unpublished_staging()
                .await
                .expect("reconcile"),
            1
        );
    }

    #[tokio::test]
    async fn reconciler_deletes_only_unreferenced_published_blobs_allowed_by_policy() {
        let blobs = InMemoryArtifactBlobStore::default();
        let payload = b"orphaned admitted payload";
        let digest = sha256_digest(payload);
        let staged = blobs
            .stage(&digest, RHAI_MEDIA_TYPE, payload)
            .await
            .expect("stage blob");
        blobs.publish(&staged).await.expect("publish blob");
        let reconciler = ArtifactAdmissionReconciler::new(blobs, RecoveryStore(Vec::new()));

        assert_eq!(
            reconciler
                .delete_unreferenced_published(&AllowBlobDeletion)
                .await
                .expect("reconcile"),
            1
        );
    }

    #[tokio::test]
    async fn retention_snapshot_requires_an_explicit_eligible_rule_for_deletion() {
        let now = Utc::now();
        let digest = sha256_digest(b"retained artifact payload");
        let policy = SnapshotArtifactBlobRetentionPolicy::new(now, HashMap::new());

        assert!(!policy
            .may_delete(&digest)
            .await
            .expect("missing retention rule fails closed"));

        let policy = SnapshotArtifactBlobRetentionPolicy::new(
            now,
            HashMap::from([(
                digest.clone(),
                ArtifactBlobRetentionRule {
                    retain_until: now,
                    legal_hold: false,
                    rollback_protected: false,
                    audit_retained: false,
                },
            )]),
        );

        assert!(policy
            .may_delete(&digest)
            .await
            .expect("expired unprotected rule allows deletion"));
    }

    #[tokio::test]
    async fn digest_pinned_package_installs_without_changing_server_source() {
        let package = package(ArtifactPayloadKind::Rhai);
        let reference = package.reference.clone();
        let store = CapturingStore::default();
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        );

        let admission = installer
            .admit(admission_command(
                reference,
                ModuleInstallationScope::Platform,
            ))
            .await
            .expect("admission");
        assert!(admission.created);
        let installed = store.0.lock().expect("store lock")[0].clone();

        assert_eq!(installed.release.slug, "sample_module");
        assert_eq!(installed.descriptor.payload_kind, ArtifactPayloadKind::Rhai);
    }

    #[tokio::test]
    async fn admission_registers_only_descriptor_permissions_without_role_grants() {
        let mut package = package(ArtifactPayloadKind::Rhai);
        package.descriptor.permissions = vec![crate::ArtifactPermissionDescriptor {
            key: "sample_module.events.handle".to_string(),
            localizations: vec![rustok_api::ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "Handle events".to_string(),
                description: "Allows handling admitted sample events".to_string(),
            }],
        }];
        let reference = package.reference.clone();
        let registrar = RecordingArtifactPermissionRegistrar::default();
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            CapturingStore::default(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            registrar.clone(),
        );

        let result = installer
            .admit(admission_command(
                reference,
                ModuleInstallationScope::Tenant {
                    tenant_id: Uuid::new_v4(),
                },
            ))
            .await
            .expect("admission");
        let registrations = registrar.0.lock().expect("registrar lock");
        assert_eq!(registrations.len(), 1);
        assert_eq!(registrations[0].installation_id, result.installation_id);
        assert_eq!(registrations[0].permissions.len(), 1);
        assert_eq!(
            registrations[0].permissions[0].key,
            "sample_module.events.handle"
        );
    }

    #[tokio::test]
    async fn static_promotion_is_not_a_runtime_install_path() {
        let package = package(ArtifactPayloadKind::StaticPromoted);
        let reference = package.reference.clone();
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            CapturingStore::default(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        );

        assert!(matches!(
            installer
                .admit(admission_command(
                    reference,
                    ModuleInstallationScope::Platform,
                ))
                .await,
            Err(ModuleInstallationError::StaticPromotionRequired)
        ));
    }

    #[tokio::test]
    async fn admission_rejects_payload_larger_than_the_configured_limit() {
        let package = package(ArtifactPayloadKind::Rhai);
        let reference = package.reference.clone();
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            CapturingStore::default(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .with_admission_limits(ArtifactAdmissionLimits {
            max_descriptor_bytes: 1024,
            max_payload_bytes: 1,
        });

        assert!(matches!(
            installer
                .admit(admission_command(
                    reference,
                    ModuleInstallationScope::Platform,
                ))
                .await,
            Err(ModuleInstallationError::ArtifactTooLarge {
                kind: "payload",
                ..
            })
        ));
    }

    #[tokio::test]
    async fn installation_only_builds_requests_for_declared_capabilities() {
        let package = package(ArtifactPayloadKind::Rhai);
        let reference = package.reference.clone();
        let ArtifactPayloadSource::Bytes(payload) = package.payload.clone() else {
            panic!("fixture uses an in-memory payload")
        };
        let scope = ModuleInstallationScope::Tenant {
            tenant_id: Uuid::new_v4(),
        };
        let store = CapturingStore::default();
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        );
        let admission = installer
            .admit(admission_command(reference, scope.clone()))
            .await
            .expect("admission");
        assert!(admission.created);
        let installed = store.0.lock().expect("store lock")[0].clone();

        let request = installed
            .sandbox_request(
                payload,
                SandboxContext::new(ExecutionPhase::Event),
                json!({ "topic": "module.installed", "payload": { "value": 42 } }),
                SandboxPolicy {
                    grants: vec![CapabilityGrant {
                        name: CapabilityName::new("platform.events").expect("capability"),
                        constraints: json!({
                            "topics": ["module.installed"],
                            "operations": ["publish"]
                        }),
                    }],
                    ..Default::default()
                },
            )
            .expect("request");

        assert!(matches!(
            request.subject,
            SandboxSubject::ModuleArtifact { installation_id, .. }
                if installation_id == installed.installation_id
        ));
        assert_eq!(
            request.payload.executor,
            rustok_sandbox::SandboxExecutorKind::Rhai
        );
        assert_eq!(
            RhaiBindingInput::decode(request.input)
                .expect("versioned Rhai input")
                .input,
            json!({ "topic": "module.installed", "payload": { "value": 42 } })
        );
        assert_eq!(installed.scope, scope);
    }

    #[tokio::test]
    async fn sea_orm_store_persists_scoped_digest_pinned_installation() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration");
        let module = crate::ModulesModule;
        for migration in module.migrations() {
            migration
                .up(&SchemaManager::new(&database))
                .await
                .expect("migration");
        }

        let package = package(ArtifactPayloadKind::Rhai);
        let reference = package.reference.clone();
        let expected_reference = package.reference.clone();
        let expected_descriptor = package.descriptor.clone();
        let expected_lock = empty_dependency_lock();
        let tenant_id = Uuid::new_v4();
        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        );
        let command = ArtifactAdmissionCommand {
            reference,
            scope: ModuleInstallationScope::Tenant { tenant_id },
            dependency_lock: expected_lock.clone(),
            sandbox_policy: SandboxPolicy::default(),
            actor_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        };
        let conflicting_command = ArtifactAdmissionCommand {
            reference: OciArtifactReference {
                registry: "registry.example".to_string(),
                repository: "modules/sample_module".to_string(),
                digest: format!("sha256:{}", "b".repeat(64)),
            },
            scope: command.scope.clone(),
            dependency_lock: command.dependency_lock.clone(),
            sandbox_policy: command.sandbox_policy.clone(),
            actor_id: command.actor_id,
            idempotency_key: command.idempotency_key,
        };
        let admission = installer.admit(command.clone()).await.expect("admission");
        assert!(admission.created);
        let duplicate = installer
            .admit(command)
            .await
            .expect("idempotent admission");
        assert!(!duplicate.created);
        assert_eq!(duplicate.installation_id, admission.installation_id);
        assert!(matches!(
            installer.admit(conflicting_command).await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        let installed = InstalledModuleArtifact {
            installation_id: admission.installation_id,
            scope: ModuleInstallationScope::Tenant { tenant_id },
            reference: expected_reference,
            release: expected_descriptor.release_ref(),
            descriptor: expected_descriptor,
            dependency_lock: expected_lock,
            capability_grant_revision: 1,
            installed_at: Utc::now(),
        };

        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT installation_id, scope_kind, tenant_id, manifest_digest, payload_digest, \
                 dependency_graph_revision, dependency_graph_digest \
                 FROM module_artifact_installations"
                    .to_string(),
            ))
            .await
            .expect("query")
            .expect("row");
        assert_eq!(
            String::try_get(&row, "", "installation_id").expect("installation id"),
            installed.installation_id.to_string()
        );
        assert_eq!(
            String::try_get(&row, "", "scope_kind").expect("scope"),
            "tenant"
        );
        assert_eq!(
            String::try_get(&row, "", "tenant_id").expect("tenant id"),
            tenant_id.to_string()
        );
        assert_eq!(
            String::try_get(&row, "", "manifest_digest").expect("manifest digest"),
            installed.reference.digest
        );
        assert_eq!(
            String::try_get(&row, "", "payload_digest").expect("payload digest"),
            installed.descriptor.artifact_digest
        );
        assert_eq!(
            i64::try_get(&row, "", "dependency_graph_revision").expect("dependency graph revision"),
            installed.dependency_lock.graph_revision as i64
        );
        assert_eq!(
            String::try_get(&row, "", "dependency_graph_digest").expect("dependency graph digest"),
            installed.dependency_lock.graph_digest
        );
        let admission = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT installation_id, payload_digest FROM module_artifact_admissions"
                    .to_string(),
            ))
            .await
            .expect("admission query")
            .expect("admission row");
        assert_eq!(
            String::try_get(&admission, "", "installation_id").expect("admission installation id"),
            installed.installation_id.to_string()
        );
        assert_eq!(
            String::try_get(&admission, "", "payload_digest").expect("admission digest"),
            installed.descriptor.artifact_digest
        );
        let admission_command_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM module_artifact_admission_commands".to_string(),
            ))
            .await
            .expect("admission command query")
            .expect("admission command count");
        assert_eq!(
            i64::try_get(&admission_command_count, "", "count").expect("admission command count"),
            1
        );
        let outbox_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sys_events WHERE event_type = 'module.artifact.admitted'"
                    .to_string(),
            ))
            .await
            .expect("outbox query")
            .expect("outbox count");
        assert_eq!(
            i64::try_get(&outbox_count, "", "count").expect("outbox count"),
            1
        );

        let evidence = ArtifactVerificationEvidence {
            manifest_digest: installed.reference.digest.clone(),
            payload_digest: installed.descriptor.artifact_digest.clone(),
            media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            signer_identity: "test.signer.example".to_string(),
            trust_policy_revision: 2,
            capability_policy_revision: 2,
            signature_verified: true,
            provenance_verified: true,
            sbom_verified: true,
            evidence_references: vec!["test://verification/reverified".to_string()],
            verified_at: Utc::now(),
        };
        assert_eq!(
            store
                .reverify_admission(ArtifactAdmissionReverification {
                    installation_id: installed.installation_id,
                    scope: ModuleInstallationScope::Tenant { tenant_id },
                    expected_revision: 1,
                    evidence: evidence.clone(),
                })
                .await
                .expect("reverification"),
            2
        );
        let reverification_outbox_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sys_events \
                 WHERE event_type = 'module.artifact.reverified'"
                    .to_string(),
            ))
            .await
            .expect("reverification outbox query")
            .expect("reverification outbox count");
        assert_eq!(
            i64::try_get(&reverification_outbox_count, "", "count")
                .expect("reverification outbox count"),
            1
        );
        assert_eq!(
            store
                .record_migration_checkpoint(ArtifactMigrationCheckpointRequest {
                    installation_id: installed.installation_id,
                    scope: ModuleInstallationScope::Tenant { tenant_id },
                    expected_revision: 2,
                    checkpoint: json!({ "data_contract_revision": 2 }),
                    has_irreversible_migration: true,
                })
                .await
                .expect("migration checkpoint"),
            3
        );
        let checkpoint_outbox_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sys_events \
                 WHERE event_type = 'module.artifact.migration_checkpointed'"
                    .to_string(),
            ))
            .await
            .expect("checkpoint outbox query")
            .expect("checkpoint outbox count");
        assert_eq!(
            i64::try_get(&checkpoint_outbox_count, "", "count").expect("checkpoint outbox count"),
            1
        );
        let checkpoint_row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT migration_checkpoint, has_irreversible_migration \
                 FROM module_artifact_installations"
                    .to_string(),
            ))
            .await
            .expect("checkpoint query")
            .expect("checkpoint row");
        assert_eq!(
            String::try_get(&checkpoint_row, "", "migration_checkpoint")
                .expect("migration checkpoint"),
            r#"{"data_contract_revision":2}"#
        );
        assert_eq!(
            i64::try_get(&checkpoint_row, "", "has_irreversible_migration")
                .expect("irreversible migration"),
            1
        );
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = 'active' WHERE installation_id = ?1"
                    .to_string(),
                vec![installed.installation_id.to_string().into()],
            ))
            .await
            .expect("activate test installation");
        let deactivation_request = ArtifactDeactivationRequest {
            installation_id: installed.installation_id,
            scope: ModuleInstallationScope::Tenant { tenant_id },
            expected_revision: 3,
            actor_id: Uuid::new_v4(),
            reason: "retire runtime bindings".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        let deactivated = store
            .deactivate_artifact(deactivation_request.clone())
            .await
            .expect("deactivate artifact");
        assert_eq!(deactivated.revision, 4);
        let conflicting_deactivation = ArtifactDeactivationRequest {
            actor_id: Uuid::new_v4(),
            ..deactivation_request.clone()
        };
        assert!(matches!(
            store.deactivate_artifact(conflicting_deactivation).await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        assert_eq!(
            store
                .deactivate_artifact(deactivation_request)
                .await
                .expect("idempotent deactivation"),
            deactivated
        );
        let deactivation_outbox_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sys_events \
                 WHERE event_type = 'module.artifact.deactivated'"
                    .to_string(),
            ))
            .await
            .expect("deactivation outbox query")
            .expect("deactivation outbox count");
        assert_eq!(
            i64::try_get(&deactivation_outbox_count, "", "count")
                .expect("deactivation outbox count"),
            1
        );
        let deactivation_row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT status, revision FROM module_artifact_admissions".to_string(),
            ))
            .await
            .expect("deactivation query")
            .expect("deactivation row");
        assert_eq!(
            String::try_get(&deactivation_row, "", "status").expect("deactivation status"),
            "inactive"
        );
        assert_eq!(
            i64::try_get(&deactivation_row, "", "revision").expect("deactivation revision"),
            4
        );
        let tenant_disable = ArtifactTenantDisableRequest {
            installation_id: installed.installation_id,
            tenant_id,
            expected_revision: 1,
            actor_id: Uuid::new_v4(),
            reason: "disable tenant intent".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert_eq!(
            store
                .disable_artifact_for_tenant(tenant_disable.clone())
                .await
                .expect("disable tenant artifact")
                .revision,
            1
        );
        let conflicting_tenant_disable = ArtifactTenantDisableRequest {
            reason: "a different disable reason".to_string(),
            ..tenant_disable.clone()
        };
        assert!(matches!(
            store
                .disable_artifact_for_tenant(conflicting_tenant_disable)
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        assert_eq!(
            store
                .disable_artifact_for_tenant(tenant_disable.clone())
                .await
                .expect("idempotent tenant disable")
                .revision,
            1
        );
        assert!(matches!(
            store
                .enable_artifact_for_tenant(ArtifactTenantEnableRequest {
                    installation_id: installed.installation_id,
                    tenant_id,
                    expected_revision: 1,
                    actor_id: tenant_disable.actor_id,
                    reason: tenant_disable.reason.clone(),
                    idempotency_key: tenant_disable.idempotency_key,
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        let tenant_enable = ArtifactTenantEnableRequest {
            installation_id: installed.installation_id,
            tenant_id,
            expected_revision: 1,
            actor_id: Uuid::new_v4(),
            reason: "restore tenant intent".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        assert_eq!(
            store
                .enable_artifact_for_tenant(tenant_enable.clone())
                .await
                .expect("enable tenant artifact")
                .revision,
            2
        );
        assert_eq!(
            store
                .enable_artifact_for_tenant(tenant_enable)
                .await
                .expect("idempotent tenant enable")
                .revision,
            2
        );
        let tenant_disable_outbox = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sys_events WHERE event_type = 'module.artifact.tenant_disabled'".to_string(),
            ))
            .await
            .expect("tenant disable outbox query")
            .expect("tenant disable outbox row");
        assert_eq!(
            i64::try_get(&tenant_disable_outbox, "", "count").expect("tenant disable outbox count"),
            1
        );
        let tenant_enable_outbox = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sys_events WHERE event_type = 'module.artifact.tenant_enabled'".to_string(),
            ))
            .await
            .expect("tenant enable outbox query")
            .expect("tenant enable outbox row");
        assert_eq!(
            i64::try_get(&tenant_enable_outbox, "", "count").expect("tenant enable outbox count"),
            1
        );
        let uninstall_request = ArtifactUninstallRequest {
            installation_id: installed.installation_id,
            scope: ModuleInstallationScope::Tenant { tenant_id },
            expected_revision: 4,
            actor_id: Uuid::new_v4(),
            reason: "remove inactive selection".to_string(),
            idempotency_key: Uuid::new_v4(),
        };
        let uninstalled = store
            .uninstall_artifact(uninstall_request.clone())
            .await
            .expect("uninstall inactive artifact");
        assert_eq!(uninstalled.revision, 5);
        let conflicting_uninstall = ArtifactUninstallRequest {
            installation_id: Uuid::new_v4(),
            ..uninstall_request.clone()
        };
        assert!(matches!(
            store.uninstall_artifact(conflicting_uninstall).await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        assert_eq!(
            store
                .uninstall_artifact(uninstall_request)
                .await
                .expect("idempotent uninstall"),
            uninstalled
        );
        let uninstall_outbox_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sys_events \
                 WHERE event_type = 'module.artifact.uninstalled'"
                    .to_string(),
            ))
            .await
            .expect("uninstall outbox query")
            .expect("uninstall outbox count");
        assert_eq!(
            i64::try_get(&uninstall_outbox_count, "", "count").expect("uninstall outbox count"),
            1
        );
        assert!(matches!(
            store
                .reverify_admission(ArtifactAdmissionReverification {
                    installation_id: installed.installation_id,
                    scope: ModuleInstallationScope::Tenant {
                        tenant_id: Uuid::new_v4(),
                    },
                    expected_revision: 4,
                    evidence,
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
    }

    #[tokio::test]
    async fn rollback_replays_an_exact_command_after_the_source_state_changes() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration");
        let module = crate::ModulesModule;
        for migration in module.migrations() {
            migration
                .up(&SchemaManager::new(&database))
                .await
                .expect("migration");
        }

        let predecessor_package = package(ArtifactPayloadKind::Rhai);
        let mut successor_package = predecessor_package.clone();
        successor_package.reference.digest = format!("sha256:{}", "b".repeat(64));
        successor_package.descriptor.version = "2.0.0".to_string();
        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let predecessor = ModuleInstaller::new(
            FixtureRegistry(predecessor_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            predecessor_package.reference.clone(),
            ModuleInstallationScope::Platform,
        ))
        .await
        .expect("predecessor admission");
        let successor = ModuleInstaller::new(
            FixtureRegistry(successor_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            successor_package.reference.clone(),
            ModuleInstallationScope::Platform,
        ))
        .await
        .expect("successor admission");

        let request = ArtifactRollbackRequest {
            installation_id: successor.installation_id,
            scope: ModuleInstallationScope::Platform,
            expected_revision: 1,
            actor_id: Uuid::new_v4(),
            reason: "restore predecessor after failed upgrade".to_string(),
            idempotency_key: Uuid::new_v4(),
            target_capability_grant_revision: 7,
            migration_rollback_mode: ArtifactMigrationRollbackMode::Reversible,
        };
        let result = store
            .rollback_artifact(request.clone())
            .await
            .expect("rollback");
        assert_eq!(result.target_installation_id, predecessor.installation_id);
        assert_eq!(result.source_revision, 2);
        assert_eq!(result.target_revision, 2);
        assert_eq!(
            store
                .rollback_artifact(request.clone())
                .await
                .expect("idempotent rollback"),
            result
        );
        assert!(matches!(
            store
                .rollback_artifact(ArtifactRollbackRequest {
                    target_capability_grant_revision: 8,
                    ..request
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        let rollback_event_count = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM sys_events \
                 WHERE event_type = 'module.artifact.rolled_back'"
                    .to_string(),
            ))
            .await
            .expect("rollback outbox query")
            .expect("rollback outbox row");
        assert_eq!(
            i64::try_get(&rollback_event_count, "", "count").expect("rollback event count"),
            1
        );
    }

    #[tokio::test]
    async fn runtime_resolver_prefers_tenant_artifact_and_honors_tenant_disable() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&database))
            .await
            .expect("outbox migration");
        let module = crate::ModulesModule;
        for migration in module.migrations() {
            migration
                .up(&SchemaManager::new(&database))
                .await
                .expect("migration");
        }

        let package = package(ArtifactPayloadKind::Rhai);
        let release = package.descriptor.release_ref();
        let tenant_id = Uuid::new_v4();
        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let platform = ModuleInstaller::new(
            FixtureRegistry(package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            package.reference.clone(),
            ModuleInstallationScope::Platform,
        ))
        .await
        .expect("platform admission");
        let tenant = ModuleInstaller::new(
            FixtureRegistry(package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            package.reference.clone(),
            ModuleInstallationScope::Tenant { tenant_id },
        ))
        .await
        .expect("tenant admission");
        database
            .execute(Statement::from_string(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = 'active'".to_string(),
            ))
            .await
            .expect("activate admissions");

        let selected = crate::ArtifactInstallationResolver::resolve(&store, &release, tenant_id)
            .await
            .expect("tenant resolution");
        assert_eq!(selected.installation_id, tenant.installation_id);
        assert_eq!(
            selected.scope,
            ModuleInstallationScope::Tenant { tenant_id }
        );
        let policies = SeaOrmArtifactSandboxPolicyResolver::new(database.clone());
        let policy = crate::ArtifactSandboxPolicyResolver::resolve(&policies, &selected, tenant_id)
            .await
            .expect("tenant sandbox policy");
        assert!(policy.grants.is_empty());

        store
            .disable_artifact_for_tenant(ArtifactTenantDisableRequest {
                installation_id: tenant.installation_id,
                tenant_id,
                expected_revision: 1,
                actor_id: Uuid::new_v4(),
                reason: "tenant supersedes this artifact".to_string(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("disable tenant artifact");
        let selected = crate::ArtifactInstallationResolver::resolve(&store, &release, tenant_id)
            .await
            .expect("platform fallback");
        assert_eq!(selected.installation_id, platform.installation_id);
        assert!(
            crate::ArtifactSandboxPolicyResolver::resolve(&policies, &selected, tenant_id)
                .await
                .is_ok()
        );

        store
            .disable_artifact_for_tenant(ArtifactTenantDisableRequest {
                installation_id: platform.installation_id,
                tenant_id,
                expected_revision: 1,
                actor_id: Uuid::new_v4(),
                reason: "tenant disables the platform artifact".to_string(),
                idempotency_key: Uuid::new_v4(),
            })
            .await
            .expect("disable platform artifact");
        assert!(
            crate::ArtifactInstallationResolver::resolve(&store, &release, tenant_id)
                .await
                .is_err()
        );
    }
}
