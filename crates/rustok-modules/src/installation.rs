use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, TryGetable,
    Value as SqlValue,
};
use semver::{Version, VersionReq};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeSet, HashMap};
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::OutboxTransport;
use rustok_sandbox::{
    SandboxContext, SandboxPayload, SandboxPolicy, SandboxRequest, SandboxSubject,
};

use crate::{
    ArtifactModuleKind, ArtifactPayloadKind, ArtifactReleaseRef, ModuleArtifactDescriptor,
    ModuleArtifactError, ModuleDependencyLockGraph,
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
    pub verified_at: DateTime<Utc>,
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
    staged: Mutex<HashMap<Uuid, (String, String, Vec<u8>)>>,
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
                (
                    staged.digest.clone(),
                    staged.media_type.clone(),
                    bytes.to_vec(),
                ),
            );
        Ok(staged)
    }

    async fn publish(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError> {
        let (_, _, bytes) = self
            .staged
            .lock()
            .map_err(|_| ModuleInstallationError::Blob("blob store lock poisoned".into()))?
            .remove(&staged.stage_id)
            .ok_or_else(|| ModuleInstallationError::Blob("staged blob is unavailable".into()))?;
        self.put_verified(&staged.digest, &bytes).await
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
}

#[async_trait]
impl ArtifactAdmissionStore for SeaOrmArtifactInstallationStore {
    async fn commit_admission(
        &self,
        artifact: &InstalledModuleArtifact,
        staged: &StagedArtifactBlob,
    ) -> Result<(), ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &artifact.scope).await?;
        let backend = transaction.get_database_backend();
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                installation_insert_sql(backend),
                installation_values(artifact, backend)?,
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                admission_insert_sql(backend),
                admission_values(artifact, staged, backend)?,
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
                "SELECT payload_digest FROM module_artifact_admissions".to_string(),
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
        DbBackend::Postgres => (1..=17)
            .map(|index| format!("${index}"))
            .collect::<Vec<_>>(),
        _ => (1..=17)
            .map(|index| format!("?{index}"))
            .collect::<Vec<_>>(),
    };
    format!(
        "INSERT INTO module_artifact_installations (\
            installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
            slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
            dependency_graph_revision, dependency_graph_digest, dependency_lock, installed_at\
         ) VALUES ({})",
        placeholders.join(", ")
    )
}

fn installation_values(
    artifact: &InstalledModuleArtifact,
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
    ])
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
    backend: DbBackend,
) -> Result<Vec<SqlValue>, ModuleInstallationError> {
    let committed_at = Utc::now();
    let evidence = serde_json::to_value(ArtifactVerificationEvidence {
        manifest_digest: artifact.reference.digest.clone(),
        payload_digest: staged.digest.clone(),
        media_type: staged.media_type.clone(),
        verified_at: committed_at,
    })
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
        self.store.commit_admission(artifact, &staged).await
    }
}

impl<R, S, B> ModuleInstaller<R, S, B>
where
    R: ArtifactRegistry,
    S: ArtifactAdmissionStore,
    B: DurableArtifactBlobStore,
{
    pub fn new(registry: R, store: S, blobs: B) -> Self {
        Self {
            registry,
            admission: ArtifactAdmissionService::new(store, blobs),
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
        let release = package.release_ref();
        let artifact = InstalledModuleArtifact {
            installation_id: Uuid::new_v4(),
            scope,
            reference: package.reference,
            release,
            descriptor: package.descriptor,
            dependency_lock,
            installed_at,
        };
        self.admission
            .admit(&artifact, &package.media_type, &package.payload)
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
    #[error("admitted artifact blob `{0}` is unavailable")]
    BlobNotFound(String),
    #[error("artifact dependency lock is invalid: {0}")]
    DependencyLock(String),
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
    use std::sync::Mutex;

    use chrono::Utc;
    use rustok_core::MigrationSource;
    use rustok_sandbox::{CapabilityGrant, CapabilityName, ExecutionPhase};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use serde_json::json;

    use super::*;

    struct FixtureRegistry(ModuleArtifactPackage);

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
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            SeaOrmArtifactInstallationStore::new(database.clone()),
            InMemoryArtifactBlobStore::default(),
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
    }
}
