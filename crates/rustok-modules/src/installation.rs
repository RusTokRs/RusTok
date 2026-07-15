use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value as SqlValue,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::OutboxTransport;
use rustok_sandbox::{
    SandboxContext, SandboxPayload, SandboxPolicy, SandboxRequest, SandboxSubject,
};

use crate::{
    ArtifactPayloadKind, ArtifactReleaseRef, ModuleArtifactDescriptor, ModuleArtifactError,
    ModuleDependencyLockGraph, TrustPolicyRevision, TrustVerificationDecision,
    TrustVerificationRequest, TrustVerifier,
};

const RHAI_MEDIA_TYPE: &str = "application/vnd.rustok.rhai.source.v1";
const WASM_COMPONENT_MEDIA_TYPE: &str = "application/wasm";
const SIDECAR_MEDIA_TYPE: &str = "application/vnd.rustok.sidecar.v1";
const STATIC_PROMOTION_MEDIA_TYPE: &str = "application/vnd.rustok.static-promotion.v1";

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

/// Bytes resolved from an OCI artifact manifest after layer selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleArtifactPackage {
    pub reference: OciArtifactReference,
    pub descriptor: ModuleArtifactDescriptor,
    pub media_type: String,
    pub payload: Vec<u8>,
}

impl ModuleArtifactPackage {
    /// Verifies artifact identity before it can enter a tenant or platform runtime.
    pub fn verify(&self, limits: ArtifactAdmissionLimits) -> Result<(), ModuleInstallationError> {
        self.reference.validate()?;
        self.descriptor.validate()?;
        limits.validate_payload_size(self.payload.len() as u64)?;
        let actual_digest = sha256_digest(&self.payload);
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

        Ok(SandboxRequest {
            subject: SandboxSubject::ModuleArtifact {
                slug: self.release.slug.clone(),
                version: self.release.version.clone(),
                digest: self.release.digest.clone(),
            },
            context,
            payload: SandboxPayload {
                executor,
                media_type: media_type_for(self.descriptor.payload_kind).to_string(),
                digest: self.release.digest.clone(),
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
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRollbackResult {
    pub operation_id: Uuid,
    pub target_installation_id: Uuid,
    pub source_revision: u64,
    pub target_revision: u64,
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
    async fn commit_admission(
        &self,
        artifact: &InstalledModuleArtifact,
        staged: &StagedArtifactBlob,
        evidence: &ArtifactVerificationEvidence,
    ) -> Result<(), ModuleInstallationError>;
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

impl SeaOrmArtifactInstallationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Removes one inactive scope selection while retaining immutable evidence
    /// for rollback/audit and deferred CAS retention.
    pub async fn uninstall_artifact(
        &self,
        request: ArtifactUninstallRequest,
    ) -> Result<ArtifactUninstallResult, ModuleInstallationError> {
        if request.expected_revision == 0 || request.reason.trim().is_empty() {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "uninstall requires a positive revision and non-empty reason".into(),
            ));
        }
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
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback requires positive revisions and a non-empty reason".into(),
            ));
        }
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = transaction.get_database_backend();
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2"),
            _ => ("?1", "?2"),
        };
        let row = transaction.query_one(Statement::from_sql_and_values(
            backend,
            format!("SELECT previous_installation_id FROM module_artifact_installations WHERE installation_id = {}", placeholders.0),
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
        let source = transaction.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE module_artifact_admissions SET status = 'rolled_back', revision = revision + 1 WHERE installation_id = {} AND revision = {}", placeholders.0, placeholders.1),
            vec![uuid_value(request.installation_id, backend), i64::try_from(request.expected_revision).map_err(|_| ModuleInstallationError::AdmissionRevisionConflict("rollback revision exceeds database range".into()))?.into()],
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
        let target_revision = target_expected_revision + 1;
        transaction.execute(Statement::from_sql_and_values(
            backend,
            format!("UPDATE module_artifact_installations SET capability_grant_revision = {} WHERE installation_id = {}", placeholders.0, placeholders.1),
            vec![i64::try_from(request.target_capability_grant_revision).map_err(|_| ModuleInstallationError::AdmissionRevisionConflict("capability grant revision exceeds database range".into()))?.into(), uuid_value(target_installation_id, backend)],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let operation_id = Uuid::new_v4();
        transaction.execute(Statement::from_sql_and_values(
            backend,
            match backend { DbBackend::Postgres => "INSERT INTO module_artifact_rollback_operations (operation_id, installation_id, target_installation_id, expected_revision, actor_id, reason, idempotency_key, committed_at) VALUES ($1,$2,$3,$4,$5,$6,$7,NOW())", _ => "INSERT INTO module_artifact_rollback_operations (operation_id, installation_id, target_installation_id, expected_revision, actor_id, reason, idempotency_key, committed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,datetime('now'))" }.to_string(),
            vec![uuid_value(operation_id, backend), uuid_value(request.installation_id, backend), uuid_value(target_installation_id, backend), i64::try_from(request.expected_revision).map_err(|_| ModuleInstallationError::AdmissionRevisionConflict("rollback revision exceeds database range".into()))?.into(), uuid_value(request.actor_id, backend), request.reason.into(), uuid_value(request.idempotency_key, backend)],
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
            source_revision: request.expected_revision + 1,
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
    async fn commit_admission(
        &self,
        artifact: &InstalledModuleArtifact,
        staged: &StagedArtifactBlob,
        evidence: &ArtifactVerificationEvidence,
    ) -> Result<(), ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &artifact.scope).await?;
        let backend = transaction.get_database_backend();
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
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))
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
pub struct ModuleInstaller<R, S, B> {
    registry: R,
    admission: ArtifactAdmissionService<S, B>,
    verifier: Arc<dyn TrustVerifier>,
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

    pub async fn admit(
        &self,
        artifact: &InstalledModuleArtifact,
        media_type: &str,
        payload: &[u8],
        evidence: &ArtifactVerificationEvidence,
    ) -> Result<(), ModuleInstallationError> {
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
            .commit_admission(artifact, &staged, evidence)
            .await
    }
}

impl<R, S, B> ModuleInstaller<R, S, B>
where
    R: ArtifactRegistry,
    S: ArtifactAdmissionStore,
    B: DurableArtifactBlobStore,
{
    pub fn new(
        registry: R,
        store: S,
        blobs: B,
        verifier: Arc<dyn TrustVerifier>,
        trust_policy: TrustPolicyRevision,
    ) -> Self {
        Self {
            registry,
            admission: ArtifactAdmissionService::new(store, blobs),
            verifier,
            trust_policy,
            limits: ArtifactAdmissionLimits::default(),
        }
    }

    pub fn with_admission_limits(mut self, limits: ArtifactAdmissionLimits) -> Self {
        self.limits = limits;
        self
    }

    pub async fn install(
        &self,
        reference: OciArtifactReference,
        scope: ModuleInstallationScope,
        dependency_lock: ModuleDependencyLockGraph,
        installed_at: DateTime<Utc>,
    ) -> Result<InstalledModuleArtifact, ModuleInstallationError> {
        reference.validate()?;
        let package = self.registry.fetch(&reference, self.limits).await?;
        if package.reference != reference {
            return Err(ModuleInstallationError::RegistryIdentityMismatch {
                requested: reference.canonical(),
                received: package.reference.canonical(),
            });
        }
        package.verify(self.limits)?;
        if package.descriptor.payload_kind == ArtifactPayloadKind::StaticPromoted {
            return Err(ModuleInstallationError::StaticPromotionRequired);
        }
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
        let artifact = InstalledModuleArtifact {
            installation_id: Uuid::new_v4(),
            scope,
            reference: package.reference,
            release,
            descriptor: package.descriptor,
            dependency_lock,
            capability_grant_revision: self.trust_policy.capability_grant_revision,
            installed_at,
        };
        let evidence = ArtifactVerificationEvidence::from_decision(
            &artifact,
            &package.media_type,
            decision,
            installed_at,
        );
        self.admission
            .admit(&artifact, &package.media_type, &package.payload, &evidence)
            .await?;
        Ok(artifact)
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::Utc;
    use rustok_core::MigrationSource;
    use rustok_sandbox::{CapabilityGrant, CapabilityName, ExecutionPhase};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use serde_json::json;

    use super::*;
    use crate::ArtifactModuleKind;

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

    #[derive(Default)]
    struct CapturingStore(Mutex<Vec<InstalledModuleArtifact>>);

    #[async_trait]
    impl ArtifactAdmissionStore for CapturingStore {
        async fn commit_admission(
            &self,
            artifact: &InstalledModuleArtifact,
            _staged: &StagedArtifactBlob,
            _evidence: &ArtifactVerificationEvidence,
        ) -> Result<(), ModuleInstallationError> {
            self.0.lock().expect("store lock").push(artifact.clone());
            Ok(())
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
        async fn commit_admission(
            &self,
            _artifact: &InstalledModuleArtifact,
            _staged: &StagedArtifactBlob,
            _evidence: &ArtifactVerificationEvidence,
        ) -> Result<(), ModuleInstallationError> {
            Ok(())
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
                schema_version: 1,
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
                settings_schema: None,
                data_schema: None,
                ui_contributions: Vec::new(),
                persistence_contract: None,
            },
            media_type: media_type_for(kind).to_string(),
            payload,
        }
    }

    fn empty_dependency_lock() -> ModuleDependencyLockGraph {
        ModuleDependencyLockGraph::create(0, Vec::new()).expect("empty dependency lock")
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
    async fn digest_pinned_package_installs_without_changing_server_source() {
        let package = package(ArtifactPayloadKind::Rhai);
        let reference = package.reference.clone();
        let store = CapturingStore::default();
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            store,
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
        );

        let installed = installer
            .install(
                reference,
                ModuleInstallationScope::Platform,
                empty_dependency_lock(),
                Utc::now(),
            )
            .await
            .expect("install");

        assert_eq!(installed.release.slug, "sample_module");
        assert_eq!(installed.descriptor.payload_kind, ArtifactPayloadKind::Rhai);
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
        );

        assert!(matches!(
            installer
                .install(
                    reference,
                    ModuleInstallationScope::Platform,
                    empty_dependency_lock(),
                    Utc::now(),
                )
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
        )
        .with_admission_limits(ArtifactAdmissionLimits {
            max_descriptor_bytes: 1024,
            max_payload_bytes: 1,
        });

        assert!(matches!(
            installer
                .install(
                    reference,
                    ModuleInstallationScope::Platform,
                    empty_dependency_lock(),
                    Utc::now(),
                )
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
        let payload = package.payload.clone();
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            CapturingStore::default(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
        );
        let scope = ModuleInstallationScope::Tenant {
            tenant_id: Uuid::new_v4(),
        };
        let installed = installer
            .install(
                reference,
                scope.clone(),
                empty_dependency_lock(),
                Utc::now(),
            )
            .await
            .expect("install");

        let request = installed
            .sandbox_request(
                payload,
                SandboxContext::new(ExecutionPhase::Event),
                json!({ "value": 42 }),
                SandboxPolicy {
                    grants: vec![CapabilityGrant {
                        name: CapabilityName::new("platform.events").expect("capability"),
                        constraints: json!({}),
                    }],
                    ..Default::default()
                },
            )
            .expect("request");

        assert!(matches!(
            request.subject,
            SandboxSubject::ModuleArtifact { .. }
        ));
        assert_eq!(
            request.payload.executor,
            rustok_sandbox::SandboxExecutorKind::Rhai
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
        let tenant_id = Uuid::new_v4();
        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
        );
        let installed = installer
            .install(
                reference,
                ModuleInstallationScope::Tenant { tenant_id },
                empty_dependency_lock(),
                Utc::now(),
            )
            .await
            .expect("install");

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
        assert!(matches!(
            store
                .reverify_admission(ArtifactAdmissionReverification {
                    installation_id: installed.installation_id,
                    scope: ModuleInstallationScope::Tenant {
                        tenant_id: Uuid::new_v4(),
                    },
                    expected_revision: 2,
                    evidence,
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
    }
}
