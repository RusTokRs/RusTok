use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait,
    Value as SqlValue,
};
use semver::Version;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
#[cfg(any(test, feature = "test-support"))]
use std::sync::Mutex;
use thiserror::Error;
use uuid::Uuid;

use rustok_api::{
    ArtifactPermissionRegistration, ArtifactPermissionRegistrationPort, ArtifactPermissionScope,
    ReleasePermissionAdmissionRequest, ScopedPermissionProjectionRequest,
};
use rustok_events::DomainEvent;
use rustok_sandbox::{
    RhaiBindingInput, SandboxContext, SandboxPayload, SandboxPolicy, SandboxRequest, SandboxSubject,
};

use crate::artifact::parse_module_version_requirement;
use crate::backend::SupportedBackend;
use crate::{
    AdmissionReverificationPolicyRevision, ArtifactDataError, ArtifactDataMigrationCheckpointStore,
    ArtifactInstallationResolveError, ArtifactModuleKind, ArtifactPayloadKind, ArtifactReleaseRef,
    ArtifactSandboxPolicyResolveError, ArtifactSandboxPolicyResolver, ArtifactUiContributionContent,
    ControlPlaneInfrastructure, ModuleArtifactDescriptor, ModuleArtifactError,
    ModuleCommandContext, ModuleDependencyLockGraph, ModuleTransitionCoordinator,
    ModuleTransitionState, RetentionHoldKind, RetentionHoldRecord, RetentionHoldStore,
    RetentionTarget, SeaOrmSecurityEpochStore, TransitionCheckpointStore, TrustEvidenceReference,
    TrustPolicyRevision,
    TrustVerificationDecision, TrustVerificationError, TrustVerificationRequest, TrustVerifier,
};

const MAX_ARTIFACT_MIGRATION_CHECKPOINT_BYTES: usize = 16 * 1024;
/// Bounds one stale-admission candidate batch for the reverification
/// maintenance worker.
const MAX_ADMISSION_REVERIFICATION_CANDIDATES: u32 = 256;
/// Bounds how many admission rows one stale-candidate scan examines while
/// looking past fresh evidence; the worker repeats the scan per pass.
const MAX_ADMISSION_REVERIFICATION_SCAN_ROWS: u32 = 8_192;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum ArtifactUiResourceKind {
    NavigationRoute,
    StorefrontSlot,
}

impl ArtifactUiResourceKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::NavigationRoute => "navigation_route",
            Self::StorefrontSlot => "storefront_slot",
        }
    }
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct ArtifactUiResource {
    kind: ArtifactUiResourceKind,
    key: String,
}

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
    /// Builds validated admission limits. Zero is never an "unbounded" or
    /// "disabled" sentinel for these limits: both bounds must admit at least
    /// one byte, and callers that construct the public fields directly are
    /// still checked by every owner boundary through [`Self::validate`].
    pub fn new(
        max_descriptor_bytes: u64,
        max_payload_bytes: u64,
    ) -> Result<Self, ModuleInstallationError> {
        let limits = Self {
            max_descriptor_bytes,
            max_payload_bytes,
        };
        limits.validate()?;
        Ok(limits)
    }

    pub fn validate(&self) -> Result<(), ModuleInstallationError> {
        if self.max_descriptor_bytes == 0 || self.max_payload_bytes == 0 {
            return Err(ModuleInstallationError::InvalidAdmissionLimits);
        }
        Ok(())
    }

    pub fn validate_descriptor_size(&self, actual: u64) -> Result<(), ModuleInstallationError> {
        self.validate()?;
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
        self.validate()?;
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
        limits.validate()?;
        self.reference.validate()?;
        self.descriptor.validate()?;
        let (size, actual_digest) = match &self.payload {
            ArtifactPayloadSource::Bytes(bytes) => (bytes.len() as u64, sha256_digest(bytes)),
            ArtifactPayloadSource::TemporaryFile(path) => sha256_file(path).await?,
        };
        if size == 0 {
            return Err(ModuleInstallationError::EmptyPayload);
        }
        limits.validate_payload_size(size)?;
        if actual_digest != self.descriptor.artifact_digest {
            return Err(ModuleInstallationError::PayloadDigestMismatch {
                expected: self.descriptor.artifact_digest.clone(),
                actual: actual_digest,
            });
        }
        if !valid_media_type_for(self.descriptor.payload_kind, &self.media_type) {
            return Err(ModuleInstallationError::UnexpectedMediaType {
                expected: expected_media_types_for(self.descriptor.payload_kind).to_string(),
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
    /// Stable opaque identity of retained mutable artifact state. It survives
    /// a compatible update and is never derived from a module slug or version.
    pub data_owner_id: Uuid,
    /// Opaque initial data instance allocated only for a declared data boundary.
    /// Tenant owner references select the current instance after a cutover.
    pub namespace_instance_id: Option<Uuid>,
    /// Logical secret bindings have an independent instance and never follow a
    /// structured-data purge or restore implicitly.
    pub secret_instance_id: Option<Uuid>,
    /// Exact mutable settings instance selected for this installation. It is
    /// distinct from the installation so compatible release changes keep one
    /// governed settings lineage.
    pub settings_instance_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub reference: OciArtifactReference,
    pub release: ArtifactReleaseRef,
    pub descriptor: ModuleArtifactDescriptor,
    /// Exact admitted payload representation. Runtime must use this durable
    /// admission fact rather than infer a representation from payload kind.
    pub payload_media_type: String,
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

impl ModuleInstallationScope {
    /// A mutable owner command must carry the same tenant boundary as the
    /// installation/admission scope it is allowed to affect.
    pub(crate) fn matches_command_context(&self, context: &ModuleCommandContext) -> bool {
        let scope_tenant_id = match self {
            Self::Platform => None,
            Self::Tenant { tenant_id } => Some(*tenant_id),
        };
        context.tenant_id == scope_tenant_id && context.validate().is_ok()
    }
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
        if self.installation_id.is_nil()
            || self.data_owner_id.is_nil()
            || self.settings_instance_id.is_nil()
        {
            return Err(ModuleInstallationError::DependencyLock(
                "artifact installation persistence identities must be non-nil".into(),
            ));
        }
        let declares_data = self.descriptor.persistence_contract.is_some();
        let declares_secrets = self
            .descriptor
            .capabilities
            .iter()
            .any(|capability| capability.as_str() == "platform.secrets");
        if self.namespace_instance_id.is_some() != declares_data
            || self.secret_instance_id.is_some() != declares_secrets
            || self.namespace_instance_id.is_some_and(|id| id.is_nil())
            || self.secret_instance_id.is_some_and(|id| id.is_nil())
        {
            return Err(ModuleInstallationError::DependencyLock(
                "artifact instance identities must match declared mutable boundaries".into(),
            ));
        }
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
                parse_module_version_requirement(&dependency.version_requirement).map_err(|error| {
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
        if !valid_media_type_for(self.descriptor.payload_kind, &self.payload_media_type) {
            return Err(ModuleInstallationError::UnexpectedMediaType {
                expected: expected_media_types_for(self.descriptor.payload_kind).to_string(),
                actual: self.payload_media_type.clone(),
            });
        }
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
                media_type: self.payload_media_type.clone(),
                digest: self.release.digest.clone(),
                runtime_abi: self.descriptor.runtime_abi.clone(),
                entrypoint: self.descriptor.entrypoint.clone(),
                bytes: payload,
            },
            input,
            rhai_scope: None,
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
    /// Best-effort discard for failure paths whose primary error is already
    /// being returned: the staged blob never crossed the publish boundary,
    /// so a discard failure must not mask that error. Callers discarding a
    /// staged blob as their primary action use [`Self::discard`] and
    /// propagate.
    async fn discard_best_effort(&self, staged: &StagedArtifactBlob) {
        let _ = self.discard(staged).await;
    }
    /// Returns only digests that have crossed the durable publish boundary.
    /// The reconciler compares this set with committed control-plane references.
    async fn published_digests(&self) -> Result<Vec<String>, ModuleInstallationError>;
    async fn delete(&self, digest: &str) -> Result<(), ModuleInstallationError>;
}

/// Best-effort removal of a persisted staging temp file on a path whose
/// primary result is already determined: a leftover file under the system
/// temp dir is janitorial, and a removal failure must not mask that
/// result. Same policy as [`DurableArtifactBlobStore::discard_best_effort`].
pub(crate) async fn remove_staged_file_best_effort(path: &std::path::Path) {
    let _ = tokio::fs::remove_file(path).await;
}

#[async_trait]
impl<B: ArtifactBlobStore + ?Sized> ArtifactBlobStore for Arc<B> {
    async fn put_verified(
        &self,
        digest: &str,
        bytes: &[u8],
    ) -> Result<(), ModuleInstallationError> {
        (**self).put_verified(digest, bytes).await
    }

    async fn get_verified(&self, digest: &str) -> Result<Vec<u8>, ModuleInstallationError> {
        (**self).get_verified(digest).await
    }
}

#[async_trait]
impl<B: DurableArtifactBlobStore + ?Sized> DurableArtifactBlobStore for Arc<B> {
    async fn stage(
        &self,
        expected_digest: &str,
        expected_media_type: &str,
        bytes: &[u8],
    ) -> Result<StagedArtifactBlob, ModuleInstallationError> {
        (**self)
            .stage(expected_digest, expected_media_type, bytes)
            .await
    }

    async fn stage_file(
        &self,
        expected_digest: &str,
        expected_media_type: &str,
        source: &std::path::Path,
    ) -> Result<StagedArtifactBlob, ModuleInstallationError> {
        (**self)
            .stage_file(expected_digest, expected_media_type, source)
            .await
    }

    async fn publish(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError> {
        (**self).publish(staged).await
    }

    async fn discard(&self, staged: &StagedArtifactBlob) -> Result<(), ModuleInstallationError> {
        (**self).discard(staged).await
    }

    async fn published_digests(&self) -> Result<Vec<String>, ModuleInstallationError> {
        (**self).published_digests().await
    }

    async fn delete(&self, digest: &str) -> Result<(), ModuleInstallationError> {
        (**self).delete(digest).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StagedArtifactBlob {
    pub stage_id: Uuid,
    pub digest: String,
    pub media_type: String,
    pub size_bytes: u64,
    #[serde(default)]
    pub staging_object_key: Option<String>,
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
    pub license_policy_verified: bool,
    pub vulnerability_policy_verified: bool,
    pub evidence: Vec<TrustEvidenceReference>,
    pub verified_at: DateTime<Utc>,
}

impl ArtifactVerificationEvidence {
    fn from_decision(
        artifact: &InstalledModuleArtifact,
        media_type: &str,
        decision: TrustVerificationDecision,
        verified_at: DateTime<Utc>,
    ) -> Self {
        Self::from_decision_parts(
            &artifact.reference,
            &artifact.descriptor,
            media_type,
            decision,
            verified_at,
        )
    }

    fn from_decision_parts(
        reference: &OciArtifactReference,
        descriptor: &ModuleArtifactDescriptor,
        media_type: &str,
        decision: TrustVerificationDecision,
        verified_at: DateTime<Utc>,
    ) -> Self {
        Self {
            manifest_digest: reference.digest.clone(),
            payload_digest: descriptor.artifact_digest.clone(),
            media_type: media_type.to_string(),
            signer_identity: decision.signer_identity,
            trust_policy_revision: decision.trust_policy_revision,
            capability_policy_revision: decision.capability_policy_revision,
            signature_verified: decision.signature_verified,
            provenance_verified: decision.provenance_verified,
            sbom_verified: decision.sbom_verified,
            license_policy_verified: decision.license_policy_verified,
            vulnerability_policy_verified: decision.vulnerability_policy_verified,
            evidence: decision.evidence,
            verified_at,
        }
    }

    /// Reconstructs the canonical verifier decision carried by this persisted
    /// evidence. Admission policy is evaluated only by
    /// [`TrustVerificationDecision`]; persisted evidence does not maintain a
    /// second copy of that logic.
    pub(crate) fn decision(&self) -> TrustVerificationDecision {
        TrustVerificationDecision {
            signer_identity: self.signer_identity.clone(),
            trust_policy_revision: self.trust_policy_revision,
            capability_policy_revision: self.capability_policy_revision,
            signature_verified: self.signature_verified,
            provenance_verified: self.provenance_verified,
            sbom_verified: self.sbom_verified,
            license_policy_verified: self.license_policy_verified,
            vulnerability_policy_verified: self.vulnerability_policy_verified,
            evidence: self.evidence.clone(),
        }
    }
}

/// Requests owner-run reverification of one stored admission after a policy or
/// trust-root change. The command deliberately contains no verification
/// evidence: [`ArtifactAdmissionReverificationService`] loads the immutable
/// OCI reference and descriptor from durable storage, invokes its injected
/// [`TrustVerifier`] under the current policy revisions, and derives the
/// persisted evidence from that decision. It never changes the admitted CAS
/// digest or descriptor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAdmissionReverificationCommand {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    /// Reverification is an owner control-plane write, not a worker report.
    /// It therefore carries the same authenticated command context as every
    /// other mutable admission command.
    pub context: ModuleCommandContext,
}

/// One durable admission whose stored verification evidence does not
/// reflect the current trust/capability policy revision, as surfaced by
/// [`ArtifactAdmissionReverificationService::list_stale_candidates`].
/// `expected_revision` is the compare-and-swap input
/// [`ArtifactAdmissionReverificationCommand`] requires; a concurrent
/// admission write simply makes the follow-up command conflict and the
/// next scan re-evaluate the row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAdmissionReverificationCandidate {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
}

/// Immutable command envelope for a rollback selection. The caller supplies
/// the capability-grant revision produced by owner policy evaluation for the
/// predecessor release; the command never reopens an OCI registry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRollbackRequest {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    pub context: ModuleCommandContext,
    pub reason: String,
    pub target_capability_grant_revision: u64,
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

/// Immutable command envelope for a transition-scoped rollback selection. The caller
/// supplies the active transition operation identity; the owner resolves the active
/// installation and its retained predecessor release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionRollbackRequest {
    pub operation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactRollbackResult {
    pub operation_id: Uuid,
    pub source_installation_id: Uuid,
    pub target_installation_id: Uuid,
    pub source_revision: u64,
    pub target_revision: u64,
}

/// Activates one scoped admitted artifact. The owner serializes all activation
/// changes for the same `(scope, slug)`, captures the sole current serving
/// predecessor, and retains it for a later direct rollback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactActivationRequest {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactActivationResult {
    pub operation_id: Uuid,
    pub predecessor_installation_id: Option<Uuid>,
    pub installation_revision: u64,
    pub predecessor_revision: Option<u64>,
}

/// Governed-transition binding for one activation executed as the
/// serving-selection step of a durable transition checkpoint.
///
/// The checkpoint must already exist in `PreStaging` for the same
/// operation. Inside the activation transaction the owner revalidates
/// the checkpoint against the locked candidate/predecessor pair and the
/// current security epoch, records the predecessor rollout-window
/// retention holds, advances the checkpoint to `Observing`, and uses the
/// checkpoint operation as the activation operation — all atomically
/// with the selection change. Digests are descriptor payload digests
/// (`ArtifactReleaseRef::digest`), matching checkpoint semantics; OCI
/// manifest digests are read from the locked rows for retention holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GovernedArtifactActivation {
    pub operation_id: Uuid,
    pub expected_candidate_digest: String,
    pub expected_predecessor_digest: Option<String>,
    pub observation_duration: std::time::Duration,
}

/// Removes runtime bindings for one active artifact selection while preserving
/// the admitted release, immutable evidence, rollback predecessor, and data.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDeactivationRequest {
    pub installation_id: Uuid,
    pub scope: ModuleInstallationScope,
    pub expected_revision: u64,
    pub context: ModuleCommandContext,
    pub reason: String,
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
    pub context: ModuleCommandContext,
    pub reason: String,
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
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTenantEnableResult {
    pub revision: u64,
}

/// Owner-issued tenant lifecycle state for one admitted Optional artifact.
///
/// `revision` is zero only when no explicit tenant intent has been persisted.
/// In that inherited state, callers must submit `expected_revision` (one) for
/// the first enable or disable command. Once an explicit intent exists, both
/// values are the same and the returned revision is the next command's CAS
/// precondition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactTenantLifecycleSnapshot {
    pub installation_id: Uuid,
    pub tenant_id: Uuid,
    pub enabled: bool,
    pub revision: u64,
    pub expected_revision: u64,
}

#[derive(Debug)]
struct ArtifactTenantLifecycleCommand {
    installation_id: Uuid,
    tenant_id: Uuid,
    expected_revision: u64,
    context: ModuleCommandContext,
    reason: String,
    enabled: bool,
}

impl From<ArtifactTenantDisableRequest> for ArtifactTenantLifecycleCommand {
    fn from(request: ArtifactTenantDisableRequest) -> Self {
        Self {
            installation_id: request.installation_id,
            tenant_id: request.tenant_id,
            expected_revision: request.expected_revision,
            context: request.context,
            reason: request.reason,
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
            context: request.context,
            reason: request.reason,
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
    pub context: ModuleCommandContext,
    pub reason: String,
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
    /// Mandatory authenticated command evidence. Checkpoints are owner
    /// commands, never guest writes, even when they record a sandbox data
    /// migration.
    pub context: ModuleCommandContext,
    pub reason: String,
}

/// Immutable owner command for one artifact admission. The complete command
/// context is control-plane evidence; it is never supplied by the untrusted
/// OCI descriptor.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArtifactAdmissionCommand {
    pub reference: OciArtifactReference,
    pub scope: ModuleInstallationScope,
    pub dependency_lock: ModuleDependencyLockGraph,
    /// Host-selected sandbox grants and limits. Descriptor declarations are
    /// validated against this policy but cannot create grants themselves.
    pub sandbox_policy: SandboxPolicy,
    pub context: ModuleCommandContext,
}

impl ArtifactAdmissionCommand {
    fn validate(&self) -> Result<(), ModuleInstallationError> {
        self.reference.validate()?;
        self.context.validate().map_err(|error| {
            ModuleInstallationError::AdmissionRevisionConflict(format!(
                "admission requires a valid command context: {error}"
            ))
        })?;
        let scope_tenant_id = match &self.scope {
            ModuleInstallationScope::Platform => None,
            ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
        };
        if self.context.tenant_id != scope_tenant_id {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission command context tenant does not match installation scope".into(),
            ));
        }
        self.dependency_lock
            .validate()
            .map_err(|error| ModuleInstallationError::DependencyLock(error.to_string()))?;
        Ok(())
    }

    /// Digest over only the semantic admission request. The durable command
    /// identity already partitions replay by scope, actor, and idempotency
    /// key; trace/correlation context is evidence and may change on retry.
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

/// Durable state of the RBAC permission projection for one admitted
/// installation.
///
/// Admission and permission projection are separate side effects owned by
/// different boundaries. `Pending` therefore remains explicit in the
/// admission result until the shared RBAC port has admitted the release
/// definitions and projected them into the installation scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactPermissionRegistrationStatus {
    NotRequired,
    Pending,
    Registered,
}

impl ArtifactPermissionRegistrationStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NotRequired => "not_required",
            Self::Pending => "pending",
            Self::Registered => "registered",
        }
    }

    fn from_store_str(value: &str) -> Result<Self, ModuleInstallationError> {
        match value {
            "not_required" => Ok(Self::NotRequired),
            "pending" => Ok(Self::Pending),
            "registered" => Ok(Self::Registered),
            _ => Err(ModuleInstallationError::Store(format!(
                "stored artifact permission registration status `{value}` is unsupported"
            ))),
        }
    }
}

fn initial_permission_registration_status(
    descriptor: &ModuleArtifactDescriptor,
) -> ArtifactPermissionRegistrationStatus {
    if descriptor.permissions.is_empty() {
        ArtifactPermissionRegistrationStatus::NotRequired
    } else {
        ArtifactPermissionRegistrationStatus::Pending
    }
}

/// Stable acknowledgement of one owner admission command. Retries return the
/// original installation identity and never create another release selection.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactAdmissionResult {
    pub installation_id: Uuid,
    pub created: bool,
    pub permission_registration: ArtifactPermissionRegistrationStatus,
}

/// Stored evidence required to replay one admission without consulting the
/// distribution registry again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactAdmissionReplay {
    pub result: ArtifactAdmissionResult,
    pub descriptor: ModuleArtifactDescriptor,
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
///
/// Publication is a two-resource saga: the CAS bytes must be durable before
/// the admission reference may commit, but that publish-to-commit window
/// must not look like an orphan to blob GC. Implementations therefore
/// record an exact staged-blob publication reservation before the bytes
/// cross the publish boundary and consume that reservation inside the
/// admission commit transaction.
#[async_trait]
pub trait ArtifactAdmissionStore: Send + Sync {
    async fn find_admission_replay(
        &self,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<Option<ArtifactAdmissionReplay>, ModuleInstallationError>;

    /// Marks the scoped RBAC permission projection complete for an admitted
    /// installation. Implementations must make an already-complete call a
    /// no-op so an admission retry can safely heal a lost acknowledgement.
    async fn mark_permission_registration_complete(
        &self,
        installation_id: Uuid,
        scope: &ModuleInstallationScope,
    ) -> Result<(), ModuleInstallationError>;

    /// Reserves one staged digest against blob collection until its
    /// admission transaction commits or the reservation lease expires.
    async fn reserve_blob_publication(
        &self,
        staged: &StagedArtifactBlob,
        reserved_at: DateTime<Utc>,
    ) -> Result<(), ModuleInstallationError>;

    /// Releases the exact staged publication reservation. `false` means it
    /// was already consumed by a commit or removed after expiry.
    async fn release_blob_publication_reservation(
        &self,
        staged: &StagedArtifactBlob,
    ) -> Result<bool, ModuleInstallationError>;

    async fn commit_admission(
        &self,
        artifact: &InstalledModuleArtifact,
        staged: &StagedArtifactBlob,
        evidence: &ArtifactVerificationEvidence,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<ArtifactAdmissionResult, ModuleInstallationError>;
    /// Returns the durable CAS digests retained by a committed installation.
    async fn referenced_blob_digests(&self) -> Result<BTreeSet<String>, ModuleInstallationError>;
}

/// Minimum time a published CAS blob must remain durably marked as
/// unreferenced before collection may be claimed. The grace period closes
/// the admission race in which bytes cross the CAS publish boundary shortly
/// before their admission transaction commits.
pub const ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS: i64 = 24 * 60 * 60;

/// Lease for one durable blob-collection claim. A claim older than this lease
/// may be reclaimed and its idempotent external delete retried.
pub const ARTIFACT_BLOB_GC_CLAIM_LEASE_SECONDS: i64 = 300;

/// Lease for one staged blob's pre-publish admission reservation. The
/// reservation is the pending reference that fences blob GC between CAS
/// publication and the admission commit; an expired reservation no longer
/// authorizes that commit, so the caller must stage and publish again.
pub const ARTIFACT_BLOB_PUBLICATION_RESERVATION_LEASE_SECONDS: i64 = 60 * 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactBlobGcCandidateState {
    Marked,
    Deleting,
}

impl ArtifactBlobGcCandidateState {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Marked => "marked",
            Self::Deleting => "deleting",
        }
    }

    fn from_stored(value: &str) -> Result<Self, ModuleInstallationError> {
        match value {
            "marked" => Ok(Self::Marked),
            "deleting" => Ok(Self::Deleting),
            other => Err(ModuleInstallationError::Store(format!(
                "unknown artifact blob GC candidate state `{other}`"
            ))),
        }
    }
}

/// Durable intent to collect one published CAS blob after its grace period.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBlobGcCandidate {
    pub digest: String,
    pub first_seen_unreferenced_at: DateTime<Utc>,
    pub grace_ends_at: DateTime<Utc>,
    pub state: ArtifactBlobGcCandidateState,
    pub revision: u64,
    pub delete_started_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactBlobGcMarkOutcome {
    Marked(ArtifactBlobGcCandidate),
    AlreadyMarked(ArtifactBlobGcCandidate),
    AlreadyDeleting(ArtifactBlobGcCandidate),
    Referenced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ArtifactBlobGcClaimOutcome {
    Claimed(ArtifactBlobGcCandidate),
    Missing,
    Referenced { revoked: bool },
    GraceNotElapsed(ArtifactBlobGcCandidate),
    ClaimInProgress(ArtifactBlobGcCandidate),
}

/// Durable candidate-store boundary for two-phase published-blob collection.
///
/// Implementations must re-evaluate committed admission references and
/// live pre-publish reservations inside the same transaction that inserts
/// a mark or transitions it to the `deleting` claim. The external CAS
/// delete happens only after that claim transaction commits; completion
/// then removes exactly the claimed revision. Admission creation must
/// fence the same candidate: a marked candidate is revoked atomically with
/// the new reference and its consumed publication reservation, while a
/// deleting candidate rejects the admission so the caller can retry and
/// republish.
#[async_trait]
pub trait ArtifactBlobGcCandidateStore: ArtifactAdmissionStore {
    async fn mark_unreferenced_blob(
        &self,
        digest: &str,
        first_seen_unreferenced_at: DateTime<Utc>,
        grace_ends_at: DateTime<Utc>,
    ) -> Result<ArtifactBlobGcMarkOutcome, ModuleInstallationError>;

    async fn claim_unreferenced_blob_gc(
        &self,
        digest: &str,
        now: DateTime<Utc>,
    ) -> Result<ArtifactBlobGcClaimOutcome, ModuleInstallationError>;

    async fn revoke_marked_blob_gc_candidate(
        &self,
        digest: &str,
    ) -> Result<bool, ModuleInstallationError>;

    async fn blob_gc_candidates(
        &self,
    ) -> Result<Vec<ArtifactBlobGcCandidate>, ModuleInstallationError>;

    async fn complete_blob_gc(
        &self,
        digest: &str,
        claimed_revision: u64,
    ) -> Result<bool, ModuleInstallationError>;
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

/// Production retention policy backed by the durable retention-hold ledger.
///
/// The hold ledger is the single owner of "these bytes must survive" facts
/// for CAS content: transitions record rollout-window and predecessor
/// standby holds against admitted payload digests, and operators record
/// investigation, audit, and legal holds against the same identities. A
/// digest with no active hold is retention-eligible; physical deletion
/// still requires the reconciler's own gates on top of this answer (the
/// blob is unreferenced by every live admission on two independent
/// checks, the durable grace period has elapsed, and a final `deleting`
/// claim commits). Loading the snapshot fails the whole pass on any
/// storage error, so a collector never deletes on partial hold knowledge.
#[derive(Debug, Clone, Default)]
pub struct SeaOrmArtifactBlobRetentionPolicy {
    held_digests: BTreeSet<String>,
}

impl SeaOrmArtifactBlobRetentionPolicy {
    /// Loads the digest set protected by currently active retention holds.
    /// Callers reload per reconciliation pass so holds recorded between
    /// passes take effect before any further deletion.
    pub async fn load_active<C: ConnectionTrait>(
        db: &C,
    ) -> Result<Self, ModuleInstallationError> {
        let records = RetentionHoldStore::list_active_holds(db)
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let mut held_digests = BTreeSet::new();
        for record in records {
            match record.target {
                RetentionTarget::SourceCasBlob { digest }
                | RetentionTarget::AdmittedPayloadCas { digest }
                | RetentionTarget::OciManifest { digest }
                | RetentionTarget::OciLayer { digest }
                | RetentionTarget::PlatformExecutableCas { digest }
                | RetentionTarget::OperationsToolPackage {
                    package_digest: digest,
                } => {
                    held_digests.insert(digest);
                }
                RetentionTarget::OciReferrer {
                    digest,
                    subject_digest,
                } => {
                    // The hold protects the referrer binding, so retain
                    // both the referrer artifact and its subject if either
                    // digest appears in the published-blob inventory.
                    held_digests.insert(digest);
                    held_digests.insert(subject_digest);
                }
                _ => {}
            }
        }
        Ok(Self { held_digests })
    }

    /// Number of CAS digests protected by the loaded hold snapshot.
    pub fn held_digest_count(&self) -> usize {
        self.held_digests.len()
    }
}

#[async_trait]
impl ArtifactBlobRetentionPolicy for SeaOrmArtifactBlobRetentionPolicy {
    async fn may_delete(&self, digest: &str) -> Result<bool, ModuleInstallationError> {
        Ok(!self.held_digests.contains(digest))
    }
}

/// Outcome of one bounded-by-storage-inventory published-blob reconciliation
/// pass. Newly discovered orphans are only marked; physical deletion belongs
/// to a later pass after the durable grace period and final reference claim.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactBlobGcReport {
    pub scanned: usize,
    pub referenced: usize,
    pub retained_by_policy: usize,
    pub newly_marked: usize,
    pub pending_grace: usize,
    pub claim_in_progress: usize,
    pub revoked: usize,
    pub raced_candidates: usize,
    pub deleted: usize,
}

impl ArtifactBlobGcReport {
    pub const fn has_changes(self) -> bool {
        self.newly_marked > 0 || self.revoked > 0 || self.deleted > 0
    }

    fn increment(counter: &mut usize) -> Result<(), ModuleInstallationError> {
        *counter = counter.checked_add(1).ok_or_else(|| {
            ModuleInstallationError::Store("artifact blob GC report counter overflow".into())
        })?;
        Ok(())
    }
}

/// Reconciles published CAS blobs through durable two-phase collection.
///
/// The first pass that observes an unreferenced blob records a tombstone and
/// leaves the bytes untouched. A later pass may delete only after retention
/// still permits it and the candidate store commits a final `deleting`
/// claim after rechecking live admission references. The external delete is
/// therefore never authorized by the initial inventory snapshot alone.
pub struct ArtifactAdmissionReconciler<B, S> {
    blobs: B,
    admissions: S,
}

impl<B, S> ArtifactAdmissionReconciler<B, S>
where
    B: DurableArtifactBlobStore,
    S: ArtifactBlobGcCandidateStore,
{
    pub fn new(blobs: B, admissions: S) -> Self {
        Self { blobs, admissions }
    }

    // There is deliberately no "discard unfinished staging" sweep: the
    // admission-command ledger does not record staged-blob identities, so a
    // truthful recovery record cannot be reconstructed from it. (The former
    // implementation fabricated blobs from idempotency keys and request
    // digests.) Staging cleanup belongs to the blob store's own TTL/GC over
    // unpublished staging objects, keyed by real stage ids.

    pub async fn reconcile_published_blobs(
        &self,
        retention: &dyn ArtifactBlobRetentionPolicy,
    ) -> Result<ArtifactBlobGcReport, ModuleInstallationError> {
        self.reconcile_published_blobs_at(retention, Utc::now())
            .await
    }

    async fn reconcile_published_blobs_at(
        &self,
        retention: &dyn ArtifactBlobRetentionPolicy,
        now: DateTime<Utc>,
    ) -> Result<ArtifactBlobGcReport, ModuleInstallationError> {
        let referenced_snapshot = self.admissions.referenced_blob_digests().await?;
        let published = self
            .blobs
            .published_digests()
            .await?
            .into_iter()
            .collect::<BTreeSet<_>>();
        let candidates = self
            .admissions
            .blob_gc_candidates()
            .await?
            .into_iter()
            .map(|candidate| (candidate.digest.clone(), candidate))
            .collect::<BTreeMap<_, _>>();
        let digests = published
            .iter()
            .cloned()
            .chain(candidates.keys().cloned())
            .collect::<BTreeSet<_>>();

        let mut report = ArtifactBlobGcReport {
            scanned: digests.len(),
            ..ArtifactBlobGcReport::default()
        };
        let grace_ends_at =
            now.checked_add_signed(chrono::Duration::seconds(
                ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS,
            ))
            .ok_or_else(|| {
                ModuleInstallationError::Store(
                    "artifact blob GC grace period exceeds the supported timestamp range".into(),
                )
            })?;

        for digest in digests {
            let candidate_snapshot = candidates.get(&digest).cloned();
            if referenced_snapshot.contains(&digest) {
                ArtifactBlobGcReport::increment(&mut report.referenced)?;
                if candidate_snapshot.is_some()
                    && self
                        .admissions
                        .revoke_marked_blob_gc_candidate(&digest)
                        .await?
                {
                    ArtifactBlobGcReport::increment(&mut report.revoked)?;
                }
                continue;
            }

            if !retention.may_delete(&digest).await? {
                ArtifactBlobGcReport::increment(&mut report.retained_by_policy)?;
                continue;
            }

            let should_claim = match candidate_snapshot {
                Some(_) => true,
                None => match self
                    .admissions
                    .mark_unreferenced_blob(&digest, now, grace_ends_at)
                    .await?
                {
                    ArtifactBlobGcMarkOutcome::Marked(_) => {
                        ArtifactBlobGcReport::increment(&mut report.newly_marked)?;
                        false
                    }
                    ArtifactBlobGcMarkOutcome::AlreadyMarked(_)
                    | ArtifactBlobGcMarkOutcome::AlreadyDeleting(_) => true,
                    ArtifactBlobGcMarkOutcome::Referenced => {
                        ArtifactBlobGcReport::increment(&mut report.referenced)?;
                        false
                    }
                },
            };
            if !should_claim {
                continue;
            }

            match self
                .admissions
                .claim_unreferenced_blob_gc(&digest, now)
                .await?
            {
                ArtifactBlobGcClaimOutcome::Claimed(candidate) => {
                    // The claim transaction has committed. No database lock
                    // is held across this external storage effect, and a
                    // crash leaves a stale claim that a later pass retries.
                    self.blobs.delete(&digest).await?;
                    if !self
                        .admissions
                        .complete_blob_gc(&digest, candidate.revision)
                        .await?
                    {
                        return Err(ModuleInstallationError::Store(format!(
                            "artifact blob GC candidate `{digest}` changed after its claimed external delete; completion was not recorded"
                        )));
                    }
                    ArtifactBlobGcReport::increment(&mut report.deleted)?;
                }
                ArtifactBlobGcClaimOutcome::Missing => {
                    ArtifactBlobGcReport::increment(&mut report.raced_candidates)?;
                }
                ArtifactBlobGcClaimOutcome::Referenced { revoked } => {
                    ArtifactBlobGcReport::increment(&mut report.referenced)?;
                    if revoked {
                        ArtifactBlobGcReport::increment(&mut report.revoked)?;
                    }
                }
                ArtifactBlobGcClaimOutcome::GraceNotElapsed(_) => {
                    ArtifactBlobGcReport::increment(&mut report.pending_grace)?;
                }
                ArtifactBlobGcClaimOutcome::ClaimInProgress(_) => {
                    ArtifactBlobGcReport::increment(&mut report.claim_in_progress)?;
                }
            }
        }

        Ok(report)
    }
}

/// Test/local adapter. Production adapters must use durable storage outside
/// PostgreSQL and preserve the same digest verification invariant. This
/// volatile double exists only behind `test-support` so a production
/// composition cannot accidentally satisfy the durability trait with it.
#[cfg(any(test, feature = "test-support"))]
#[derive(Default)]
pub struct InMemoryArtifactBlobStore {
    blobs: Mutex<HashMap<String, Vec<u8>>>,
    staged: Mutex<HashMap<Uuid, InMemoryStagedArtifact>>,
    infrastructure: ControlPlaneInfrastructure,
}

#[cfg(any(test, feature = "test-support"))]
impl InMemoryArtifactBlobStore {
    pub fn with_infrastructure(infrastructure: ControlPlaneInfrastructure) -> Self {
        Self {
            blobs: Mutex::default(),
            staged: Mutex::default(),
            infrastructure,
        }
    }
}

#[cfg(any(test, feature = "test-support"))]
struct InMemoryStagedArtifact {
    bytes: Vec<u8>,
}

#[cfg(any(test, feature = "test-support"))]
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

#[cfg(any(test, feature = "test-support"))]
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
            stage_id: self.infrastructure.new_id(),
            digest: expected_digest.to_string(),
            media_type: expected_media_type.to_string(),
            size_bytes: bytes.len() as u64,
            staging_object_key: None,
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
    infrastructure: ControlPlaneInfrastructure,
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
        let infrastructure = ControlPlaneInfrastructure::for_database(db.clone());
        Self::with_infrastructure(db, infrastructure)
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self { db, infrastructure }
    }

    async fn begin_platform_blob_gc_transaction(
        &self,
    ) -> Result<(DatabaseTransaction, SupportedBackend), ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &ModuleInstallationScope::Platform).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok((transaction, backend))
    }

    /// Resolves the exact active installation named by a platform-routed binding.
    /// The initial identity lookup is not trusted for execution: it is reduced
    /// to an immutable release tuple and then rechecked through `resolve_exact`.
    pub async fn resolve_routed_installation(
        &self,
        installation_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<InstalledModuleArtifact, ArtifactInstallationResolveError> {
        if installation_id.is_nil() || tenant_id.is_nil() {
            return Err(ArtifactInstallationResolveError::InvalidRequest(
                "artifact route requires non-nil identities".to_string(),
            ));
        }
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &ModuleInstallationScope::Tenant { tenant_id })
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let placeholder = if backend == SupportedBackend::Postgres {
            "$1"
        } else {
            "?1"
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT slug, version, payload_digest FROM module_artifact_installations WHERE installation_id = {placeholder} LIMIT 1"
                ),
                vec![backend.uuid_value(installation_id)],
            ))
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let row = row.ok_or(ArtifactInstallationResolveError::Unavailable)?;
        let release = ArtifactReleaseRef {
            slug: row
                .try_get("", "slug")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
            version: row
                .try_get("", "version")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
            digest: row
                .try_get("", "payload_digest")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
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
        let request_digest = migration_checkpoint_request_digest(&request)?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, tenant_id) = match request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(tenant_id)),
        };
        let scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = ?2 AND installation.tenant_id IS ?3"
            }
        };
        let operation_scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = $3 AND installation.tenant_id IS NOT DISTINCT FROM $4"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = ?3 AND installation.tenant_id IS ?4"
            }
        };
        let existing = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT operation.expected_revision, operation.revision, operation.request_digest \
                     FROM module_artifact_migration_checkpoint_operations operation \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = operation.installation_id \
                     WHERE operation.installation_id = {} AND operation.idempotency_key = {} \
                       AND {operation_scope}",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" },
                    if backend == SupportedBackend::Postgres { "$2" } else { "?2" },
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    backend.uuid_value(request.context.idempotency_key),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if let Some(existing) = existing {
            let expected: i64 = existing
                .try_get("", "expected_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let revision: i64 = existing
                .try_get("", "revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let stored_digest: String = existing
                .try_get("", "request_digest")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if expected != request.expected_revision as i64 || stored_digest != request_digest {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "idempotency key was already used for a different migration checkpoint command"
                        .into(),
                ));
            }
            return positive_revision(revision, "migration checkpoint replay");
        }
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT admission.revision, admission.status, installation.has_irreversible_migration \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {} AND {scope} \
                     AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                     WHERE uninstall.installation_id = installation.installation_id)",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
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
            SupportedBackend::Postgres => row
                .try_get::<bool>("", "has_irreversible_migration")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            SupportedBackend::Sqlite => {
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
        let next_revision = positive_revision(
            revision.checked_add(1).ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "migration checkpoint revision overflow".into(),
                )
            })?,
            "migration checkpoint",
        )?;
        let operation_id = self.infrastructure.new_id();
        let operation_placeholders = match backend {
            SupportedBackend::Postgres => "$1,$2,$3,$4,$5,$6,$7,$8,$9,$10,NOW()",
            SupportedBackend::Sqlite => "?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')",
        };
        let claimed = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_migration_checkpoint_operations \
                     (operation_id, installation_id, expected_revision, revision, request_digest, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) \
                     VALUES ({operation_placeholders}) \
                     ON CONFLICT (installation_id, idempotency_key) DO NOTHING"
                ),
                vec![
                    backend.uuid_value(operation_id),
                    backend.uuid_value(request.installation_id),
                    revision.into(),
                    (next_revision as i64).into(),
                    request_digest.clone().into(),
                    backend.uuid_value(request.context.actor_id),
                    request.context.trace_id.clone().into(),
                    backend.uuid_value(request.context.correlation_id),
                    request.reason.clone().into(),
                    backend.uuid_value(request.context.idempotency_key),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if claimed.rows_affected() != 1 {
            let existing = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                        "SELECT operation.expected_revision, operation.revision, operation.request_digest \
                         FROM module_artifact_migration_checkpoint_operations operation \
                         JOIN module_artifact_installations installation \
                           ON installation.installation_id = operation.installation_id \
                         WHERE operation.installation_id = {} AND operation.idempotency_key = {} \
                           AND {operation_scope}",
                        if backend == SupportedBackend::Postgres { "$1" } else { "?1" },
                        if backend == SupportedBackend::Postgres { "$2" } else { "?2" },
                    ),
                    vec![
                        backend.uuid_value(request.installation_id),
                        backend.uuid_value(request.context.idempotency_key),
                        scope_kind.into(),
                        backend.optional_uuid_value(tenant_id),
                    ],
                ))
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                .ok_or_else(|| {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "migration checkpoint idempotency claim was not retained".into(),
                    )
                })?;
            let expected: i64 = existing
                .try_get("", "expected_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let revision: i64 = existing
                .try_get("", "revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let stored_digest: String = existing
                .try_get("", "request_digest")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if expected != request.expected_revision as i64 || stored_digest != request_digest {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "idempotency key was already used for a different migration checkpoint command"
                        .into(),
                ));
            }
            return positive_revision(revision, "migration checkpoint replay");
        }
        let checkpoint = serde_json::to_value(&request.checkpoint)
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let update_installation = match backend {
            SupportedBackend::Postgres => {
                "UPDATE module_artifact_installations SET migration_checkpoint = $1, has_irreversible_migration = has_irreversible_migration OR $2 WHERE installation_id = $3"
            }
            SupportedBackend::Sqlite => {
                "UPDATE module_artifact_installations SET migration_checkpoint = ?1, has_irreversible_migration = MAX(has_irreversible_migration, ?2) WHERE installation_id = ?3"
            }
        };
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                update_installation.to_string(),
                vec![
                    SqlValue::Json(Some(Box::new(checkpoint))),
                    request.has_irreversible_migration.into(),
                    backend.uuid_value(request.installation_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let update_admission = format!(
            "UPDATE module_artifact_admissions SET revision = revision + 1 WHERE installation_id = {} AND revision = {}",
            if backend == SupportedBackend::Postgres {
                "$1"
            } else {
                "?1"
            },
            if backend == SupportedBackend::Postgres {
                "$2"
            } else {
                "?2"
            }
        );
        if transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                update_admission,
                vec![backend.uuid_value(request.installation_id), revision.into()],
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
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactMigrationCheckpointed {
                        installation_id: request.installation_id,
                        revision: next_revision,
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
        Ok(next_revision)
    }

    /// Atomically selects this installation as the scoped serving release and
    /// freezes the exact previously active installation for direct rollback.
    ///
    /// A plain activation is fenced by any unresolved governed transition
    /// for the same slug and scope (or a platform-scope transition for a
    /// tenant request): the governed saga owns the selection until its
    /// checkpoint resolves.
    pub async fn activate_artifact(
        &self,
        request: ArtifactActivationRequest,
    ) -> Result<ArtifactActivationResult, ModuleInstallationError> {
        self.activate_artifact_inner(request, None).await
    }

    /// Governed form of [`Self::activate_artifact`]: the selection change,
    /// predecessor retention holds, and the checkpoint's
    /// `PreStaging -> Observing` advance commit in one transaction under
    /// the checkpoint's operation identity.
    pub(crate) async fn activate_artifact_governed(
        &self,
        request: ArtifactActivationRequest,
        governed: GovernedArtifactActivation,
    ) -> Result<ArtifactActivationResult, ModuleInstallationError> {
        self.activate_artifact_inner(request, Some(governed)).await
    }

    async fn activate_artifact_inner(
        &self,
        request: ArtifactActivationRequest,
        governed: Option<GovernedArtifactActivation>,
    ) -> Result<ArtifactActivationResult, ModuleInstallationError> {
        validate_lifecycle_command(
            request.installation_id,
            &request.scope,
            request.expected_revision,
            &request.context,
            &request.reason,
        )?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, tenant_id) = match request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(tenant_id)),
        };
        let scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = ?2 AND installation.tenant_id IS ?3"
            }
        };
        let existing = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT operation.operation_id, operation.installation_id, \
                     operation.predecessor_installation_id, operation.expected_revision, \
                     operation.installation_revision, operation.predecessor_revision, \
                     operation.actor_id, operation.trace_id, operation.correlation_id, operation.reason \
                     FROM module_artifact_activation_operations operation \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = operation.installation_id \
                     WHERE operation.idempotency_key = {} AND {scope}",
                    if backend == SupportedBackend::Postgres {
                        "$1"
                    } else {
                        "?1"
                    }
                ),
                vec![
                    backend.uuid_value(request.context.idempotency_key),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if let Some(existing) = existing {
            let installation_id = required_uuid_from_row(&existing, "installation_id", backend)?;
            let expected_revision: i64 = existing
                .try_get("", "expected_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if installation_id != request.installation_id
                || expected_revision != request.expected_revision as i64
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "idempotency key was already used for a different activation command".into(),
                ));
            }
            let installation_revision: i64 = existing
                .try_get("", "installation_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let predecessor_revision: Option<i64> = existing
                .try_get("", "predecessor_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let replay = ArtifactActivationResult {
                operation_id: required_uuid_from_row(&existing, "operation_id", backend)?,
                predecessor_installation_id: optional_uuid_from_row(
                    &existing,
                    "predecessor_installation_id",
                    backend,
                )?,
                installation_revision: positive_revision(installation_revision, "activation")?,
                predecessor_revision: predecessor_revision
                    .map(|revision| positive_revision(revision, "activation predecessor"))
                    .transpose()?,
            };
            // A governed activation may only replay its own checkpoint
            // operation. Replaying a plain activation under a governed
            // checkpoint would claim an observation window that was never
            // opened atomically with the selection change.
            if let Some(governed) = &governed
                && replay.operation_id != governed.operation_id
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "activation idempotency key replayed a different operation than the governed transition checkpoint".into(),
                ));
            }
            return Ok(replay);
        }

        let candidate = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.slug, installation.registry, installation.repository, \
                     installation.data_owner_id, installation.settings_instance_id, \
                     installation.namespace_instance_id, installation.secret_instance_id, \
                     CAST(installation.descriptor AS TEXT) AS descriptor, \
                     admission.status, admission.revision \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {} AND {scope} \
                       AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                       WHERE uninstall.installation_id = installation.installation_id)",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "installation is absent, uninstalled, or outside the requested scope".into(),
                )
            })?;
        let slug: String = candidate
            .try_get("", "slug")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let candidate_registry: String = candidate
            .try_get("", "registry")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let candidate_repository: String = candidate
            .try_get("", "repository")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let candidate_data_owner_id = required_uuid_from_row(&candidate, "data_owner_id", backend)?;
        let candidate_settings_instance_id =
            required_uuid_from_row(&candidate, "settings_instance_id", backend)?;
        let candidate_namespace_instance_id =
            optional_uuid_from_row(&candidate, "namespace_instance_id", backend)?;
        let candidate_secret_instance_id =
            optional_uuid_from_row(&candidate, "secret_instance_id", backend)?;
        let candidate_descriptor: ModuleArtifactDescriptor = serde_json::from_str(
            &candidate
                .try_get::<String>("", "descriptor")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        )
        .map_err(|_| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "candidate installation descriptor is invalid".into(),
            )
        })?;
        candidate_descriptor.validate().map_err(|_| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "candidate installation descriptor is invalid".into(),
            )
        })?;
        acquire_artifact_activation_lock(&transaction, &request.scope, &slug).await?;
        assert_artifact_ui_contributions_available(
            &transaction,
            &request.scope,
            &slug,
            &candidate_descriptor,
        )
        .await?;
        let status: String = candidate
            .try_get("", "status")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let candidate_revision: i64 = candidate
            .try_get("", "revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if candidate_revision != request.expected_revision as i64
            || !matches!(status.as_str(), "admitted" | "installed" | "inactive")
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "activation requires an eligible installation at the expected revision".into(),
            ));
        }
        let predecessor_installation_id =
            active_predecessor_installation(&transaction, &request.scope, &slug, backend).await?;
        let predecessor_state = if let Some(predecessor_installation_id) =
            predecessor_installation_id
        {
            let predecessor = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                            "SELECT admission.revision, installation.registry, installation.repository, \
                             installation.manifest_digest, \
                             installation.data_owner_id, installation.settings_instance_id, \
                             installation.namespace_instance_id, installation.secret_instance_id, \
                             CAST(installation.descriptor AS TEXT) AS descriptor \
                         FROM module_artifact_admissions admission \
                         JOIN module_artifact_installations installation \
                           ON installation.installation_id = admission.installation_id \
                         WHERE admission.installation_id = {} AND admission.status = 'active'",
                        if backend == SupportedBackend::Postgres {
                            "$1"
                        } else {
                            "?1"
                        }
                    ),
                    vec![backend.uuid_value(predecessor_installation_id)],
                ))
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                .ok_or_else(|| {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "serving predecessor became unavailable during activation".into(),
                    )
                })?;
            let predecessor_revision: i64 = predecessor
                .try_get("", "revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let predecessor_descriptor: ModuleArtifactDescriptor = serde_json::from_str(
                &predecessor
                    .try_get::<String>("", "descriptor")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            )
            .map_err(|_| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "serving predecessor descriptor is invalid".into(),
                )
            })?;
            predecessor_descriptor.validate().map_err(|_| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "serving predecessor descriptor is invalid".into(),
                )
            })?;
            let predecessor_registry: String = predecessor
                .try_get("", "registry")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let predecessor_repository: String = predecessor
                .try_get("", "repository")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let predecessor_manifest_digest: String = predecessor
                .try_get("", "manifest_digest")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if predecessor_registry != candidate_registry
                || predecessor_repository != candidate_repository
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                        "activation requires the predecessor repository for retained data-owner continuity".into(),
                    ));
            }
            if predecessor_descriptor.settings_schema_digest
                != candidate_descriptor.settings_schema_digest
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                        "activation requires equal artifact settings schemas until a guarded settings migration is available".into(),
                    ));
            }
            let data_owner_id = required_uuid_from_row(&predecessor, "data_owner_id", backend)?;
            let settings_instance_id =
                required_uuid_from_row(&predecessor, "settings_instance_id", backend)?;
            let namespace_instance_id = if predecessor_descriptor.persistence_contract
                == candidate_descriptor.persistence_contract
            {
                optional_uuid_from_row(&predecessor, "namespace_instance_id", backend)?
            } else {
                candidate_namespace_instance_id
            };
            let secret_instance_id = if candidate_secret_instance_id.is_some() {
                optional_uuid_from_row(&predecessor, "secret_instance_id", backend)?
                    .or(candidate_secret_instance_id)
            } else {
                None
            };
            let deactivated = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                        "UPDATE module_artifact_admissions \
                         SET status = 'inactive', revision = revision + 1 \
                         WHERE installation_id = {} AND revision = {} AND status = 'active'",
                        if backend == SupportedBackend::Postgres {
                            "$1"
                        } else {
                            "?1"
                        },
                        if backend == SupportedBackend::Postgres {
                            "$2"
                        } else {
                            "?2"
                        },
                    ),
                    vec![
                        backend.uuid_value(predecessor_installation_id),
                        predecessor_revision.into(),
                    ],
                ))
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if deactivated.rows_affected() != 1 {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "serving predecessor became stale during activation".into(),
                ));
            }
            Some((
                predecessor_revision.checked_add(1).ok_or_else(|| {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "activation predecessor revision exceeds database range".into(),
                    )
                })?,
                data_owner_id,
                settings_instance_id,
                namespace_instance_id,
                secret_instance_id,
                predecessor_descriptor.artifact_digest.clone(),
                predecessor_manifest_digest,
            ))
        } else {
            None
        };
        let predecessor_revision = predecessor_state
            .as_ref()
            .map(|(revision, _, _, _, _, _, _)| *revision);
        let predecessor_digest = predecessor_state
            .as_ref()
            .map(|(_, _, _, _, _, digest, _)| digest.clone());
        let predecessor_manifest_digest = predecessor_state
            .as_ref()
            .map(|(_, _, _, _, _, _, digest)| digest.clone());
        // The (scope, slug) activation lock is held, the candidate is at
        // its expected revision, and the exact predecessor pair is known.
        // Governed-transition fencing is decided here, before the
        // selection mutation below commits: a plain activation must not
        // bypass an unresolved governed checkpoint, and a governed
        // activation must still match the checkpoint it claims to serve.
        match &governed {
            None => {
                assert_no_unresolved_governed_checkpoint(
                    &transaction,
                    &request.scope,
                    &slug,
                    "activation",
                )
                .await?;
            }
            Some(governed) => {
                if governed.operation_id.is_nil() || governed.observation_duration.is_zero() {
                    return Err(ModuleInstallationError::AdmissionRevisionConflict(
                        "governed activation requires a non-nil operation and a positive observation window".into(),
                    ));
                }
                let checkpoint = TransitionCheckpointStore::load_checkpoint(
                    &transaction,
                    governed.operation_id,
                )
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                .ok_or_else(|| {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "governed activation checkpoint was not found".into(),
                    )
                })?;
                if checkpoint.module_slug != slug
                    || checkpoint.tenant_id != tenant_id
                    || checkpoint.candidate_digest != candidate_descriptor.artifact_digest
                    || checkpoint.predecessor_digest != predecessor_digest
                    || governed.expected_candidate_digest != candidate_descriptor.artifact_digest
                    || governed.expected_predecessor_digest != predecessor_digest
                {
                    return Err(ModuleInstallationError::AdmissionRevisionConflict(
                        "governed activation no longer matches the transition checkpoint's candidate or predecessor".into(),
                    ));
                }
                if checkpoint.state != ModuleTransitionState::PreStaging {
                    return Err(ModuleInstallationError::AdmissionRevisionConflict(format!(
                        "governed activation requires a pre-staging checkpoint, not `{}`",
                        checkpoint.state.name()
                    )));
                }
                let security_registry =
                    SeaOrmSecurityEpochStore::load_registry(&transaction)
                        .await
                        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                let mut coordinator = ModuleTransitionCoordinator::new(checkpoint);
                if let (Some(payload_digest), Some(manifest_digest)) = (
                    &predecessor_digest,
                    &predecessor_manifest_digest,
                ) {
                    let now = Utc::now();
                    let observation =
                        chrono::Duration::from_std(governed.observation_duration).map_err(|_| {
                            ModuleInstallationError::AdmissionRevisionConflict(
                                "governed activation observation window is outside the supported range".into(),
                            )
                        })?;
                    let expires_at = now
                        .checked_add_signed(observation + chrono::Duration::hours(24))
                        .ok_or_else(|| {
                            ModuleInstallationError::AdmissionRevisionConflict(
                                "governed activation retention window is outside the supported range".into(),
                            )
                        })?;
                    for target in [
                        RetentionTarget::AdmittedPayloadCas {
                            digest: payload_digest.clone(),
                        },
                        RetentionTarget::OciManifest {
                            digest: manifest_digest.clone(),
                        },
                    ] {
                        RetentionHoldStore::insert_hold(
                            &transaction,
                            &RetentionHoldRecord {
                                hold_id: Uuid::new_v4(),
                                target,
                                kind: RetentionHoldKind::ActiveRolloutWindow {
                                    operation_id: governed.operation_id,
                                    expires_at,
                                },
                                created_at: now,
                            },
                        )
                        .await
                        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                    }
                } else if predecessor_digest.is_some() {
                    return Err(ModuleInstallationError::Store(
                        "serving predecessor manifest digest is missing for the governed retention hold".into(),
                    ));
                }
                let ledger = RetentionHoldStore::load_active_ledger(&transaction)
                    .await
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                coordinator
                    .advance_to_activating_with_ledger(
                        &security_registry,
                        Some(&ledger),
                        governed.observation_duration,
                    )
                    .map_err(|error| {
                        ModuleInstallationError::AdmissionRevisionConflict(error.to_string())
                    })?;
                TransitionCheckpointStore::save_checkpoint(
                    &transaction,
                    coordinator.checkpoint(),
                )
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            }
        }
        let (data_owner_id, settings_instance_id, namespace_instance_id, secret_instance_id) =
            predecessor_state
                .as_ref()
                .map(
                    |(
                        _,
                        data_owner_id,
                        settings_instance_id,
                        namespace_instance_id,
                        secret_instance_id,
                        _,
                        _,
                    )| {
                        (
                            *data_owner_id,
                            *settings_instance_id,
                            *namespace_instance_id,
                            *secret_instance_id,
                        )
                    },
                )
                .unwrap_or((
                    candidate_data_owner_id,
                    candidate_settings_instance_id,
                    candidate_namespace_instance_id,
                    candidate_secret_instance_id,
                ));
        let installation_revision = candidate_revision.checked_add(1).ok_or_else(|| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "activation revision exceeds database range".into(),
            )
        })?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_installations SET previous_installation_id = {}, \
                     data_owner_id = {}, settings_instance_id = {}, \
                     namespace_instance_id = {}, secret_instance_id = {} \
                     WHERE installation_id = {}",
                    if backend == SupportedBackend::Postgres {
                        "$1"
                    } else {
                        "?1"
                    },
                    if backend == SupportedBackend::Postgres {
                        "$2"
                    } else {
                        "?2"
                    },
                    if backend == SupportedBackend::Postgres {
                        "$3"
                    } else {
                        "?3"
                    },
                    if backend == SupportedBackend::Postgres {
                        "$4"
                    } else {
                        "?4"
                    },
                    if backend == SupportedBackend::Postgres {
                        "$5"
                    } else {
                        "?5"
                    },
                    if backend == SupportedBackend::Postgres {
                        "$6"
                    } else {
                        "?6"
                    },
                ),
                vec![
                    backend.optional_uuid_value(predecessor_installation_id),
                    backend.uuid_value(data_owner_id),
                    backend.uuid_value(settings_instance_id),
                    backend.optional_uuid_value(namespace_instance_id),
                    backend.optional_uuid_value(secret_instance_id),
                    backend.uuid_value(request.installation_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let activated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_admissions SET status = 'active', revision = revision + 1 \
                     WHERE installation_id = {} AND revision = {} \
                       AND status IN ('admitted', 'installed', 'inactive')",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" },
                    if backend == SupportedBackend::Postgres { "$2" } else { "?2" },
                ),
                vec![backend.uuid_value(request.installation_id), candidate_revision.into()],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if activated.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "installation became stale during activation".into(),
            ));
        }
        let operation_id = governed
            .as_ref()
            .map(|governed| governed.operation_id)
            .unwrap_or_else(|| self.infrastructure.new_id());
        let values = match backend {
            SupportedBackend::Postgres => "$1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,NOW()",
            SupportedBackend::Sqlite => "?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')",
        };
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_activation_operations \
                     (operation_id, installation_id, predecessor_installation_id, expected_revision, \
                      installation_revision, predecessor_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) \
                     VALUES ({values})"
                ),
                vec![
                    backend.uuid_value(operation_id),
                    backend.uuid_value(request.installation_id),
                    backend.optional_uuid_value(predecessor_installation_id),
                    candidate_revision.into(),
                    installation_revision.into(),
                    predecessor_revision.into(),
                    backend.uuid_value(request.context.actor_id),
                    request.context.trace_id.clone().into(),
                    backend.uuid_value(request.context.correlation_id),
                    request.reason.clone().into(),
                    backend.uuid_value(request.context.idempotency_key),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;

        // A plain activation remains an immediate serving-selection
        // change whose durable activation operation is the receipt. A
        // governed activation has already advanced its checkpoint to
        // Observing and recorded predecessor retention above, inside this
        // same transaction; the predecessor also stays reachable through
        // `previous_installation_id` for direct rollback.
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactActivated {
                        installation_id: request.installation_id,
                        predecessor_installation_id,
                        revision: positive_revision(installation_revision, "activation")?,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleInstallationError::Outbox(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(ArtifactActivationResult {
            operation_id,
            predecessor_installation_id,
            installation_revision: positive_revision(installation_revision, "activation")?,
            predecessor_revision: predecessor_revision
                .map(|revision| positive_revision(revision, "activation predecessor"))
                .transpose()?,
        })
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
            &request.context,
            &request.reason,
        )?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, tenant_id) = match request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(tenant_id)),
        };
        let scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = ?2 AND installation.tenant_id IS ?3"
            }
        };
        let existing = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT operation.operation_id, operation.installation_id, \
                     operation.expected_revision, operation.actor_id, operation.trace_id, \
                     operation.correlation_id, operation.reason \
                     FROM module_artifact_deactivation_operations operation \
                     JOIN module_artifact_installations installation ON installation.installation_id = operation.installation_id \
                     WHERE operation.idempotency_key = {} AND {scope}",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    backend.uuid_value(request.context.idempotency_key),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if let Some(existing) = existing {
            let installation_id = required_uuid_from_row(&existing, "installation_id", backend)?;
            let expected: i64 = existing
                .try_get("", "expected_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if installation_id != request.installation_id
                || expected != request.expected_revision as i64
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
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.slug, admission.status, admission.revision \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {} AND {scope} \
                     AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                     WHERE uninstall.installation_id = installation.installation_id)",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
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
        assert_no_unresolved_governed_checkpoint(
            &transaction,
            &request.scope,
            &target_slug,
            "deactivation",
        )
        .await?;
        let dependents = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id <> {} AND {scope} AND admission.status = 'active'",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
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
        let operation_id = self.infrastructure.new_id();
        let placeholders = match backend {
            SupportedBackend::Postgres => "$1,$2,$3,$4,$5,$6,$7,$8,NOW()",
            SupportedBackend::Sqlite => "?1,?2,?3,?4,?5,?6,?7,?8,strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')",
        };
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_deactivation_operations \
                     (operation_id, installation_id, expected_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) \
                     VALUES ({placeholders})"
                ),
                vec![
                    backend.uuid_value(operation_id),
                    backend.uuid_value(request.installation_id),
                    revision.into(),
                    backend.uuid_value(request.context.actor_id),
                    request.context.trace_id.clone().into(),
                    backend.uuid_value(request.context.correlation_id),
                    request.reason.into(),
                    backend.uuid_value(request.context.idempotency_key),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_admissions SET status = 'inactive', revision = revision + 1 \
                     WHERE installation_id = {} AND revision = {} AND status = 'active'",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" },
                    if backend == SupportedBackend::Postgres { "$2" } else { "?2" },
                ),
                vec![backend.uuid_value(request.installation_id), revision.into()],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if updated.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "installation became stale during deactivation".into(),
            ));
        }
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
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

    /// Returns the tenant-specific intent and its next exact CAS precondition
    /// for one admitted Optional artifact. Absence of a row intentionally
    /// means inherited enabled intent, not an implicit persisted selection.
    pub async fn artifact_tenant_lifecycle_snapshot(
        &self,
        installation_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<ArtifactTenantLifecycleSnapshot, ModuleInstallationError> {
        if installation_id.is_nil() || tenant_id.is_nil() {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "artifact tenant lifecycle snapshot requires non-nil installation and tenant identities"
                    .into(),
            ));
        }

        let scope = ModuleInstallationScope::Tenant { tenant_id };
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let placeholders = match backend {
            SupportedBackend::Postgres => ("$1", "$2"),
            SupportedBackend::Sqlite => ("?1", "?2"),
        };
        let tenant_scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = 'platform' OR \
                 (installation.scope_kind = 'tenant' AND installation.tenant_id = $2)"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = 'platform' OR \
                 (installation.scope_kind = 'tenant' AND installation.tenant_id = ?2)"
            }
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT CAST(installation.descriptor AS TEXT) AS descriptor, \
                            lifecycle.enabled, lifecycle.revision \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     LEFT JOIN module_artifact_tenant_lifecycle lifecycle \
                       ON lifecycle.installation_id = installation.installation_id \
                      AND lifecycle.tenant_id = {} \
                     WHERE installation.installation_id = {} \
                       AND ({tenant_scope}) \
                       AND admission.status IN ('admitted', 'installed', 'active', 'inactive', 'rolled_back') \
                       AND NOT EXISTS ( \
                           SELECT 1 \
                           FROM module_artifact_uninstall_operations uninstall \
                           WHERE uninstall.installation_id = installation.installation_id \
                       )",
                    placeholders.1, placeholders.0,
                ),
                vec![backend.uuid_value(installation_id), backend.uuid_value(tenant_id)],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "artifact is unavailable in the requested tenant scope".into(),
                )
            })?;

        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
            &row.try_get::<String>("", "descriptor")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        )
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if descriptor.module_kind != ArtifactModuleKind::Optional {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "tenant lifecycle changes are allowed only for Optional artifacts".into(),
            ));
        }

        let enabled = match row.try_get::<Option<bool>>("", "enabled") {
            Ok(Some(enabled)) => enabled,
            Ok(None) => true,
            Err(error) => {
                return Err(ModuleInstallationError::Store(error.to_string()));
            }
        };
        let persisted_revision: Option<i64> = row
            .try_get("", "revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let revision = match persisted_revision {
            Some(revision) => u64::try_from(revision).map_err(|_| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "tenant lifecycle revision is outside the supported range".into(),
                )
            })?,
            None => 0,
        };
        if persisted_revision.is_some() && revision == 0 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "tenant lifecycle revision is outside the supported range".into(),
            ));
        }
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;

        Ok(ArtifactTenantLifecycleSnapshot {
            installation_id,
            tenant_id,
            enabled,
            revision,
            expected_revision: revision.max(1),
        })
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
            &request.context,
            &request.reason,
        )?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if let Some(revision) =
            replay_artifact_tenant_lifecycle_operation(&transaction, &request).await?
        {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(revision);
        }
        let installation_placeholder = if backend == SupportedBackend::Postgres {
            "$1"
        } else {
            "?1"
        };
        let tenant_scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = 'platform' OR \
                 (installation.scope_kind = 'tenant' AND installation.tenant_id = $2)"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = 'platform' OR \
                 (installation.scope_kind = 'tenant' AND installation.tenant_id = ?2)"
            }
        };
        let artifact = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {installation_placeholder} \
                       AND ({tenant_scope}) \
                       AND admission.status IN ('admitted', 'installed', 'active', 'inactive', 'rolled_back') \
                       AND NOT EXISTS ( \
                           SELECT 1 \
                           FROM module_artifact_uninstall_operations uninstall \
                           WHERE uninstall.installation_id = installation.installation_id \
                       )",
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    backend.uuid_value(request.tenant_id),
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

        // Revalidate that no conflicting active release transition is
        // currently executing for this module and tenant. Both activation
        // locks (platform first, then tenant — the canonical order) are
        // held across the fence and the write below, so a transition
        // created by a concurrent activation cannot slip between the
        // check and the mutation. Platform-scope transitions fence tenant
        // lifecycle too: they change what this tenant resolves.
        acquire_artifact_activation_lock(
            &transaction,
            &ModuleInstallationScope::Platform,
            &descriptor.slug,
        )
        .await?;
        acquire_artifact_activation_lock(&transaction, &scope, &descriptor.slug).await?;
        let active_checkpoints =
            TransitionCheckpointStore::list_active_checkpoints_for_slug_and_tenant_or_platform(
                &transaction,
                &descriptor.slug,
                request.tenant_id,
            )
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if let Some(active_cp) = active_checkpoints
            .into_iter()
            .find(|cp| !cp.state.is_terminal())
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(format!(
                "artifact tenant lifecycle locked: active release transition is in progress for '{}' (state: {})",
                descriptor.slug,
                active_cp.state.name(),
            )));
        }
        let placeholder = if backend == SupportedBackend::Postgres {
            "$1"
        } else {
            "?1"
        };
        let tenant_placeholder = if backend == SupportedBackend::Postgres {
            "$2"
        } else {
            "?2"
        };
        let existing = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT revision \
                     FROM module_artifact_tenant_lifecycle \
                     WHERE installation_id = {placeholder} AND tenant_id = {tenant_placeholder}"
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    backend.uuid_value(request.tenant_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let revision = if let Some(row) = existing {
            let current: i64 = row
                .try_get("", "revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
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
                SupportedBackend::Postgres => ("$1", "$2", "$3", "$4", "$5", "$6", "$7", "$8"),
                SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4", "?5", "?6", "?7", "?8"),
            };
            let enabled = if request.enabled { "TRUE" } else { "FALSE" };
            let updated = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                        "UPDATE module_artifact_tenant_lifecycle \
                         SET enabled = {enabled}, revision = revision + 1, expected_revision = {}, \
                             actor_id = {}, trace_id = {}, correlation_id = {}, reason = {} \
                         WHERE installation_id = {} AND tenant_id = {} AND revision = {}",
                        placeholders.0,
                        placeholders.1,
                        placeholders.2,
                        placeholders.3,
                        placeholders.4,
                        placeholders.5,
                        placeholders.6,
                        placeholders.7,
                    ),
                    vec![
                        (request.expected_revision as i64).into(),
                        backend.uuid_value(request.context.actor_id),
                        request.context.trace_id.clone().into(),
                        backend.uuid_value(request.context.correlation_id),
                        request.reason.clone().into(),
                        backend.uuid_value(request.installation_id),
                        backend.uuid_value(request.tenant_id),
                        current.into(),
                    ],
                ))
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if updated.rows_affected() != 1 {
                if let Some(revision) =
                    replay_artifact_tenant_lifecycle_operation(&transaction, &request).await?
                {
                    transaction
                        .commit()
                        .await
                        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                    return Ok(revision);
                }
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
                SupportedBackend::Postgres => format!(
                    "INSERT INTO module_artifact_tenant_lifecycle \
                     (installation_id, tenant_id, enabled, revision, expected_revision, actor_id, trace_id, correlation_id, reason, updated_at) \
                     VALUES ($1, $2, {enabled}, 1, $3, $4, $5, $6, $7, NOW()) \
                     ON CONFLICT (installation_id, tenant_id) DO NOTHING"
                ),
                SupportedBackend::Sqlite => format!(
                    "INSERT INTO module_artifact_tenant_lifecycle \
                     (installation_id, tenant_id, enabled, revision, expected_revision, actor_id, trace_id, correlation_id, reason, updated_at) \
                     VALUES (?1, ?2, {enabled}, 1, ?3, ?4, ?5, ?6, ?7, strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')) \
                     ON CONFLICT (installation_id, tenant_id) DO NOTHING"
                ),
            };
            let inserted = transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    insert_sql,
                    vec![
                        backend.uuid_value(request.installation_id),
                        backend.uuid_value(request.tenant_id),
                        (request.expected_revision as i64).into(),
                        backend.uuid_value(request.context.actor_id),
                        request.context.trace_id.clone().into(),
                        backend.uuid_value(request.context.correlation_id),
                        request.reason.clone().into(),
                    ],
                ))
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if inserted.rows_affected() != 1 {
                if let Some(revision) =
                    replay_artifact_tenant_lifecycle_operation(&transaction, &request).await?
                {
                    transaction
                        .commit()
                        .await
                        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                    return Ok(revision);
                }
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "tenant lifecycle became stale during creation".into(),
                ));
            }
            1
        };
        record_artifact_tenant_lifecycle_operation(
            &transaction,
            self.infrastructure.new_id(),
            &request,
            revision,
        )
        .await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
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
            &request.context,
            &request.reason,
        )?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, tenant_id) = match request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(tenant_id)),
        };
        let scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = ?2 AND installation.tenant_id IS ?3"
            }
        };
        let existing = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT uninstall.operation_id, uninstall.installation_id, \
                     uninstall.expected_revision, uninstall.actor_id, uninstall.trace_id, \
                     uninstall.correlation_id, uninstall.reason \
                     FROM module_artifact_uninstall_operations uninstall \
                     JOIN module_artifact_installations installation ON installation.installation_id = uninstall.installation_id \
                     WHERE uninstall.idempotency_key = {} AND {scope}",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    backend.uuid_value(request.context.idempotency_key),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        if let Some(existing) = existing {
            let installation_id = required_uuid_from_row(&existing, "installation_id", backend)?;
            let expected: i64 = existing
                .try_get("", "expected_revision")
                .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
            if installation_id != request.installation_id
                || expected != request.expected_revision as i64
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
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.slug, admission.status, admission.revision \
             FROM module_artifact_installations installation \
             JOIN module_artifact_admissions admission \
               ON admission.installation_id = installation.installation_id \
             WHERE installation.installation_id = {} AND {scope} \
               AND NOT EXISTS ( \
                   SELECT 1 \
                   FROM module_artifact_uninstall_operations uninstall \
                   WHERE uninstall.installation_id = installation.installation_id \
               )",
                    if backend == SupportedBackend::Postgres {
                        "$1"
                    } else {
                        "?1"
                    }
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "installation is absent, uninstalled, or outside the requested scope".into(),
                )
            })?;
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
        assert_no_unresolved_governed_checkpoint(
            &transaction,
            &request.scope,
            &target_slug,
            "uninstall",
        )
        .await?;
        let dependents = transaction
            .query_all_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.slug, CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id <> {} AND {scope} AND admission.status = 'active'",
                    if backend == SupportedBackend::Postgres { "$1" } else { "?1" }
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
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
        let operation_id = self.infrastructure.new_id();
        let p = if backend == SupportedBackend::Postgres {
            "$1,$2,$3,$4,$5,$6,$7,$8,NOW()"
        } else {
            "?1,?2,?3,?4,?5,?6,?7,?8,strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')"
        };
        transaction.execute_raw(Statement::from_sql_and_values(backend.db_backend(), format!("INSERT INTO module_artifact_uninstall_operations (operation_id, installation_id, expected_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) VALUES ({p})"), vec![backend.uuid_value(operation_id), backend.uuid_value(request.installation_id), revision.into(), backend.uuid_value(request.context.actor_id), request.context.trace_id.clone().into(), backend.uuid_value(request.context.correlation_id), request.reason.into(), backend.uuid_value(request.context.idempotency_key)])).await.map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        let updated = transaction.execute_raw(Statement::from_sql_and_values(backend.db_backend(), format!("UPDATE module_artifact_admissions SET revision = revision + 1 WHERE installation_id = {} AND revision = {} AND status = 'inactive'", if backend == SupportedBackend::Postgres { "$1" } else { "?1" }, if backend == SupportedBackend::Postgres { "$2" } else { "?2" }), vec![backend.uuid_value(request.installation_id), revision.into()])).await.map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        if updated.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "installation became stale during uninstall".into(),
            ));
        }
        // Advance and retire the work generation and clear tenant intent in
        // the same transaction as the uninstall record: after this commit,
        // delivery admission must never admit work for the uninstalled
        // installation again. The upsert also retires installations that
        // were admitted before generation rows were written at admission
        // time (they retire from implicit generation 1 to 2).
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                work_generation_retire_sql(backend),
                vec![backend.uuid_value(request.installation_id)],
            ))
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "DELETE FROM module_artifact_tenant_lifecycle WHERE installation_id = {}",
                    if backend == SupportedBackend::Postgres {
                        "$1"
                    } else {
                        "?1"
                    }
                ),
                vec![backend.uuid_value(request.installation_id)],
            ))
            .await
            .map_err(|e| ModuleInstallationError::Store(e.to_string()))?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
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
        validate_lifecycle_command(
            request.installation_id,
            &request.scope,
            request.expected_revision,
            &request.context,
            &request.reason,
        )?;
        if request.target_capability_grant_revision == 0 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback requires a positive target capability-grant revision".into(),
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
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, tenant_id) = match &request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
        };
        let scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = ?2 AND installation.tenant_id IS ?3"
            }
        };
        let existing = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT operation.operation_id, operation.installation_id, \
                     operation.target_installation_id, operation.expected_revision, \
                     operation.actor_id, operation.trace_id, operation.correlation_id, operation.reason, \
                     operation.target_capability_grant_revision, \
                     operation.migration_rollback_mode, operation.source_revision, \
                     operation.target_revision \
                     FROM module_artifact_rollback_operations operation \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = operation.installation_id \
                     WHERE operation.idempotency_key = {} AND {scope}",
                    if backend == SupportedBackend::Postgres {
                        "$1"
                    } else {
                        "?1"
                    }
                ),
                vec![
                    backend.uuid_value(request.context.idempotency_key),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
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
            let stored_grant_revision: Option<i64> = existing
                .try_get("", "target_capability_grant_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let source_revision: Option<i64> = existing
                .try_get("", "source_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            let target_revision: Option<i64> = existing
                .try_get("", "target_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if installation_id != request.installation_id
                || stored_expected_revision != expected_revision
                || stored_grant_revision != Some(target_capability_grant_revision)
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
                source_installation_id: installation_id,
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
            SupportedBackend::Postgres => ("$1", "$2"),
            SupportedBackend::Sqlite => ("?1", "?2"),
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.previous_installation_id, \
                 installation.has_irreversible_migration, installation.slug, \
                 admission.status, admission.revision \
                 FROM module_artifact_installations installation \
                 JOIN module_artifact_admissions admission \
                   ON admission.installation_id = installation.installation_id \
                 WHERE installation.installation_id = {} AND {scope}",
                    placeholders.0
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "rollback predecessor is unavailable".into(),
                )
            })?;
        let source_slug: String = row
            .try_get("", "slug")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let source_status: String = row
            .try_get("", "status")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let source_current_revision: i64 = row
            .try_get("", "revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if source_status != "active" || source_current_revision != expected_revision {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback source must be active at the expected revision".into(),
            ));
        }
        acquire_artifact_activation_lock(&transaction, &request.scope, &source_slug).await?;
        let target_installation_id =
            optional_uuid_from_row(&row, "previous_installation_id", backend)?.ok_or_else(
                || {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "rollback predecessor is unavailable".into(),
                    )
                },
            )?;
        let has_irreversible_migration = match backend {
            SupportedBackend::Postgres => row
                .try_get::<bool>("", "has_irreversible_migration")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            SupportedBackend::Sqlite => {
                row.try_get::<i64>("", "has_irreversible_migration")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                    != 0
            }
        };
        let effective_migration_rollback_mode = if has_irreversible_migration {
            ArtifactMigrationRollbackMode::Prohibited
        } else {
            ArtifactMigrationRollbackMode::Reversible
        };
        if effective_migration_rollback_mode == ArtifactMigrationRollbackMode::Prohibited {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback is prohibited by the recorded data-migration policy".into(),
            ));
        }
        let target_revision_row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT admission.revision, admission.status, installation.slug, \
                     CAST(installation.descriptor AS TEXT) AS descriptor \
                     FROM module_artifact_admissions admission \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = admission.installation_id \
                     WHERE admission.installation_id = {} AND {scope} \
                       AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                       WHERE uninstall.installation_id = installation.installation_id)",
                    placeholders.0,
                ),
                vec![
                    backend.uuid_value(target_installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
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
        let target_status: String = target_revision_row
            .try_get("", "status")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let target_slug: String = target_revision_row
            .try_get("", "slug")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if target_slug != source_slug
            || !matches!(
                target_status.as_str(),
                "admitted" | "installed" | "inactive" | "rolled_back"
            )
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback predecessor is not an eligible scoped release".into(),
            ));
        }
        let target_descriptor: ModuleArtifactDescriptor = serde_json::from_str(
            &target_revision_row
                .try_get::<String>("", "descriptor")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        )
        .map_err(|_| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "rollback predecessor descriptor is invalid".into(),
            )
        })?;
        target_descriptor.validate().map_err(|_| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "rollback predecessor descriptor is invalid".into(),
            )
        })?;
        assert_artifact_ui_contributions_available(
            &transaction,
            &request.scope,
            &target_slug,
            &target_descriptor,
        )
        .await?;
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
        let transition_row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT operation_id, predecessor_installation_id \
                     FROM module_artifact_activation_operations \
                     WHERE installation_id = {} ORDER BY committed_at DESC, installation_revision DESC, operation_id DESC LIMIT 1",
                    placeholders.0,
                ),
                vec![backend.uuid_value(request.installation_id)],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "rollback source has no durable activation transition".into(),
                )
            })?;
        let transition_operation_id =
            required_uuid_from_row(&transition_row, "operation_id", backend)?;
        let recorded_predecessor =
            optional_uuid_from_row(&transition_row, "predecessor_installation_id", backend)?;
        if recorded_predecessor != Some(target_installation_id) {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback target does not match the activation transition predecessor".into(),
            ));
        }
        // Plain activations intentionally have no rollout checkpoint. When
        // an explicitly governed rollout checkpoint exists for the exact
        // activation operation, the same transaction must also record its
        // single predecessor recovery; when none exists, the durable
        // activation operation and predecessor link above are the complete
        // rollback authority.
        let transition_checkpoint =
            TransitionCheckpointStore::load_checkpoint(&transaction, transition_operation_id)
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let transition = match transition_checkpoint {
            Some(transition_checkpoint) => {
                if transition_checkpoint.module_slug != source_slug
                    || transition_checkpoint.tenant_id != tenant_id
                {
                    return Err(ModuleInstallationError::AdmissionRevisionConflict(
                        "rollback transition checkpoint scope does not match the source installation"
                            .into(),
                    ));
                }
                let mut transition =
                    crate::ModuleTransitionCoordinator::new(transition_checkpoint);
                transition
                    .record_predecessor_recovery(request.reason.clone())
                    .map_err(|error| {
                        ModuleInstallationError::AdmissionRevisionConflict(error.to_string())
                    })?;
                Some(transition)
            }
            None => None,
        };
        let operation_id = self.infrastructure.new_id();
        transaction.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            match backend {
                SupportedBackend::Postgres => "INSERT INTO module_artifact_rollback_operations (operation_id, installation_id, target_installation_id, expected_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, target_capability_grant_revision, migration_rollback_mode, source_revision, target_revision, committed_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,NOW())",
                SupportedBackend::Sqlite => "INSERT INTO module_artifact_rollback_operations (operation_id, installation_id, target_installation_id, expected_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, target_capability_grant_revision, migration_rollback_mode, source_revision, target_revision, committed_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now'))",
            }.to_string(),
            vec![
                backend.uuid_value(operation_id),
                backend.uuid_value(request.installation_id),
                backend.uuid_value(target_installation_id),
                expected_revision.into(),
                backend.uuid_value(request.context.actor_id),
                request.context.trace_id.clone().into(),
                backend.uuid_value(request.context.correlation_id),
                request.reason.clone().into(),
                backend.uuid_value(request.context.idempotency_key),
                target_capability_grant_revision.into(),
                effective_migration_rollback_mode.as_str().into(),
                source_revision.into(),
                target_revision.into(),
            ],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let source = transaction.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!("UPDATE module_artifact_admissions SET status = 'rolled_back', revision = revision + 1 WHERE installation_id = {} AND revision = {}", placeholders.0, placeholders.1),
            vec![backend.uuid_value(request.installation_id), expected_revision.into()],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if source.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback source is missing or stale".into(),
            ));
        }
        let target = transaction.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!("UPDATE module_artifact_admissions SET status = 'active', revision = revision + 1 WHERE installation_id = {} AND revision = {} AND status IN ('admitted', 'installed', 'inactive', 'rolled_back')", placeholders.0, placeholders.1),
            vec![backend.uuid_value(target_installation_id), target_expected_revision.into()],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if target.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback predecessor is not activatable".into(),
            ));
        }
        transaction.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!("UPDATE module_artifact_installations SET capability_grant_revision = {} WHERE installation_id = {}", placeholders.0, placeholders.1),
            vec![target_capability_grant_revision.into(), backend.uuid_value(target_installation_id)],
        )).await.map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if let Some(transition) = transition.as_ref() {
            TransitionCheckpointStore::save_checkpoint(&transaction, transition.checkpoint())
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        }
        RetentionHoldStore::release_holds_for_operation(&transaction, transition_operation_id)
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
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
            source_installation_id: request.installation_id,
            target_installation_id,
            source_revision: positive_revision(source_revision, "rollback source")?,
            target_revision: positive_revision(target_revision, "rollback target")?,
        })
    }

    /// Rolls back an active transition to its direct retained predecessor release.
    ///
    /// The owner resolves the active installation, its active admission CAS revision,
    /// and the direct predecessor capability grant revision directly from durable
    /// activation records before delegating to `rollback_artifact`.
    pub async fn rollback_transition_operation(
        &self,
        request: TransitionRollbackRequest,
    ) -> Result<ArtifactRollbackResult, ModuleInstallationError> {
        let trimmed_reason = request.reason.trim();
        if trimmed_reason.is_empty() {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback reason must not be empty".into(),
            ));
        }
        if request.operation_id.is_nil() || request.context.idempotency_key.is_nil() {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback requires non-nil operation and idempotency identities".into(),
            ));
        }

        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let placeholders = match backend {
            SupportedBackend::Postgres => ("$1", "$2"),
            SupportedBackend::Sqlite => ("?1", "?2"),
        };

        let op_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation_id, predecessor_installation_id \
                     FROM module_artifact_activation_operations \
                     WHERE operation_id = {}",
                    placeholders.0
                ),
                vec![backend.uuid_value(request.operation_id)],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "transition activation operation not found".into(),
                )
            })?;

        let installation_id = required_uuid_from_row(&op_row, "installation_id", backend)?;
        let predecessor_installation_id =
            optional_uuid_from_row(&op_row, "predecessor_installation_id", backend)?.ok_or_else(
                || {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "transition has no retained direct predecessor to rollback to".into(),
                    )
                },
            )?;

        let adm_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT revision, status FROM module_artifact_admissions WHERE installation_id = {}",
                    placeholders.0
                ),
                vec![backend.uuid_value(installation_id)],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "active installation admission record not found".into(),
                )
            })?;

        let status: String = adm_row
            .try_get("", "status")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if status != "active" {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "rollback candidate is no longer active".into(),
            ));
        }

        let current_revision: i64 = adm_row
            .try_get("", "revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let expected_revision = positive_revision(current_revision, "candidate admission")?;

        let grant_row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT capability_grant_revision FROM module_artifact_installations WHERE installation_id = {}",
                    placeholders.0
                ),
                vec![backend.uuid_value(predecessor_installation_id)],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;

        // The predecessor's grant revision is part of the rollback's
        // authorization terms. A missing predecessor row or a non-positive
        // stored revision is an invariant violation, never a reason to
        // fabricate revision 1 and roll back under terms that may not exist.
        let grant_row = grant_row.ok_or_else(|| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "rollback predecessor installation record not found".into(),
            )
        })?;
        let target_capability_grant_revision = positive_revision(
            grant_row
                .try_get("", "capability_grant_revision")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            "rollback predecessor capability grant",
        )?;

        self.rollback_artifact(ArtifactRollbackRequest {
            installation_id,
            scope: request.scope,
            expected_revision,
            context: request.context,
            reason: trimmed_reason.to_string(),
            target_capability_grant_revision,
        })
        .await
    }

    /// Returns a previously committed reverification result before any
    /// verifier work is attempted. A retry therefore remains available even
    /// when the isolated verifier is temporarily unavailable.
    async fn find_admission_reverification_replay(
        &self,
        request: &ArtifactAdmissionReverificationCommand,
        request_digest: &str,
        resulting_revision: u64,
    ) -> Result<Option<u64>, ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, scope_tenant_key) = admission_reverification_scope(&request.scope)?;
        let Some(existing) = find_admission_reverification_receipt(
            &transaction,
            backend,
            scope_kind,
            &scope_tenant_key,
            request.context.actor_id,
            request.context.idempotency_key,
        )
        .await?
        else {
            return Ok(None);
        };
        replay_admission_reverification(existing, request, request_digest, resulting_revision)
            .map(Some)
    }

    /// Loads the only artifact identity a reverification may evaluate. The
    /// caller supplies an installation id and revision, never a substitute
    /// reference, descriptor, media type, or evidence object.
    async fn load_admission_reverification_target(
        &self,
        request: &ArtifactAdmissionReverificationCommand,
    ) -> Result<StoredAdmissionReverificationTarget, ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, tenant_id) = match request.scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(tenant_id)),
        };
        let scope = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = ?2 AND installation.tenant_id IS ?3"
            }
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.registry, installation.repository, \
                     installation.manifest_digest, installation.slug, installation.version, \
                     installation.payload_digest AS installation_payload_digest, \
                     CAST(installation.descriptor AS TEXT) AS descriptor, \
                     admission.payload_digest AS admission_payload_digest, \
                     admission.media_type, admission.status, admission.revision \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {} AND {scope} \
                       AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                       WHERE uninstall.installation_id = installation.installation_id)",
                    backend.placeholder(1),
                ),
                vec![
                    backend.uuid_value(request.installation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
            .ok_or_else(|| {
                ModuleInstallationError::AdmissionRevisionConflict(
                    "installation is absent, uninstalled, or outside the requested scope".into(),
                )
            })?;

        let reference = OciArtifactReference {
            registry: row
                .try_get("", "registry")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            repository: row
                .try_get("", "repository")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            digest: row
                .try_get("", "manifest_digest")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        };
        reference.validate()?;
        let stored_slug: String = row
            .try_get("", "slug")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let stored_version: String = row
            .try_get("", "version")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let installation_payload_digest: String = row
            .try_get("", "installation_payload_digest")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
            &row.try_get::<String>("", "descriptor")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        )
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        descriptor
            .validate()
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let admission_payload_digest: String = row
            .try_get("", "admission_payload_digest")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let media_type: String = row
            .try_get("", "media_type")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let status: String = row
            .try_get("", "status")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let revision: i64 = row
            .try_get("", "revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let revision = positive_revision(revision, "admission reverification")?;

        if descriptor.slug != stored_slug
            || descriptor.version != stored_version
            || descriptor.artifact_digest != installation_payload_digest
            || descriptor.artifact_digest != admission_payload_digest
            || !valid_digest(&admission_payload_digest)
            || !valid_media_type_for(descriptor.payload_kind, &media_type)
        {
            return Err(ModuleInstallationError::Store(
                "stored admission identity does not match its immutable installation descriptor"
                    .into(),
            ));
        }
        if revision != request.expected_revision {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission reverification expected revision is stale".into(),
            ));
        }
        if !admission_reverification_status_allowed(&status) {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(format!(
                "admission in status '{status}' cannot be reverified"
            )));
        }
        ensure_no_active_transition_for_reverification(
            &transaction,
            &request.scope,
            &descriptor.slug,
        )
        .await?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;

        Ok(StoredAdmissionReverificationTarget {
            reference,
            descriptor,
            payload_digest: admission_payload_digest,
            media_type,
        })
    }

    /// Persists evidence derived by [`ArtifactAdmissionReverificationService`]
    /// from a current-policy verifier decision. This storage operation is not
    /// a public evidence-submission boundary.
    async fn persist_admission_reverification(
        &self,
        request: &ArtifactAdmissionReverificationCommand,
        target: &StoredAdmissionReverificationTarget,
        evidence: &ArtifactVerificationEvidence,
        admitted: bool,
        request_digest: &str,
    ) -> Result<u64, ModuleInstallationError> {
        if evidence.manifest_digest != target.reference.digest
            || evidence.payload_digest != target.payload_digest
            || evidence.payload_digest != target.descriptor.artifact_digest
            || evidence.media_type != target.media_type
        {
            return Err(ModuleInstallationError::Store(
                "owner-derived reverification evidence does not match the stored admission identity"
                    .into(),
            ));
        }
        let expected_revision = i64::try_from(request.expected_revision).map_err(|_| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "expected revision exceeds database integer range".into(),
            )
        })?;
        let resulting_revision = request.expected_revision.checked_add(1).ok_or_else(|| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "resulting revision exceeds database integer range".into(),
            )
        })?;
        let resulting_revision_i64 = i64::try_from(resulting_revision).map_err(|_| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "resulting revision exceeds database integer range".into(),
            )
        })?;
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &request.scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, scope_tenant_key) = admission_reverification_scope(&request.scope)?;
        if let Some(existing) = find_admission_reverification_receipt(
            &transaction,
            backend,
            scope_kind,
            &scope_tenant_key,
            request.context.actor_id,
            request.context.idempotency_key,
        )
        .await?
        {
            return replay_admission_reverification(
                existing,
                request,
                request_digest,
                resulting_revision,
            );
        }
        ensure_no_active_transition_for_reverification(
            &transaction,
            &request.scope,
            &target.descriptor.slug,
        )
        .await?;
        let receipt_inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                admission_reverification_receipt_insert_sql(backend),
                vec![
                    scope_kind.into(),
                    scope_tenant_key.clone().into(),
                    backend.uuid_value(request.context.actor_id),
                    backend.uuid_value(request.context.idempotency_key),
                    request.context.trace_id.clone().into(),
                    backend.uuid_value(request.context.correlation_id),
                    request_digest.to_owned().into(),
                    backend.uuid_value(request.installation_id),
                    expected_revision.into(),
                    resulting_revision_i64.into(),
                    backend.datetime_value(self.infrastructure.now()),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if receipt_inserted.rows_affected() != 1 {
            let existing = find_admission_reverification_receipt(
                &transaction,
                backend,
                scope_kind,
                &scope_tenant_key,
                request.context.actor_id,
                request.context.idempotency_key,
            )
            .await?
            .ok_or_else(|| {
                ModuleInstallationError::Store(
                    "admission reverification receipt disappeared after an idempotency conflict"
                        .into(),
                )
            })?;
            return replay_admission_reverification(
                existing,
                request,
                request_digest,
                resulting_revision,
            );
        }
        let evidence = serde_json::to_value(evidence)
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let status = if admitted {
            ArtifactAdmissionStatus::Admitted
        } else {
            ArtifactAdmissionStatus::Failed
        };
        let placeholders = match backend {
            SupportedBackend::Postgres => ("$1", "$2", "$3", "$4", "$5", "$6"),
            SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4", "?5", "?6"),
        };
        let (scope_predicate, scope_values) = admission_scope_predicate(&request.scope, backend);
        let sql = format!(
            "UPDATE module_artifact_admissions \
             SET verification_evidence = {}, status = {}, revision = revision + 1 \
             WHERE installation_id = {} AND revision = {} \
               AND payload_digest = {} AND media_type = {} \
               AND status IN ('admitted', 'installed', 'inactive', 'failed') \
               AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                               WHERE uninstall.installation_id = module_artifact_admissions.installation_id)",
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
            backend.uuid_value(request.installation_id),
            expected_revision.into(),
            target.payload_digest.clone().into(),
            target.media_type.clone().into(),
        ];
        values.extend(scope_values);
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                sql,
                values,
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if updated.rows_affected() != 1 {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission is missing, stale, uninstalled, or no longer eligible for reverification"
                    .into(),
            ));
        }
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &request.context,
                    DomainEvent::ModuleArtifactReverified {
                        installation_id: request.installation_id,
                        status: status.as_str().to_string(),
                        revision: resulting_revision,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleInstallationError::Outbox(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(resulting_revision)
    }

    /// Finds admissions whose durable verification evidence does not reflect
    /// the current trust/capability policy revisions, for the maintenance
    /// worker that re-verifies them under current policy. Only statuses the
    /// reverification write accepts are considered. Evidence that no longer
    /// decodes is treated as stale on purpose: the write replaces evidence
    /// wholesale, so routing the row through reverification is its only
    /// healing path, and corrupt evidence must never exempt an admission
    /// from current-policy checks.
    ///
    /// The scan keyset-paginates by installation id inside one
    /// platform-scoped transaction and stops after `limit` candidates or
    /// `MAX_ADMISSION_REVERIFICATION_SCAN_ROWS` examined rows, whichever
    /// comes first, so a pass stays bounded while still looking past any
    /// fresh prefix. Corrupt scope or revision values fail the pass loudly
    /// instead of being skipped silently.
    pub async fn list_stale_admission_reverification_candidates(
        &self,
        trust_policy: &AdmissionReverificationPolicyRevision,
        limit: u32,
    ) -> Result<Vec<ArtifactAdmissionReverificationCandidate>, ModuleInstallationError> {
        if limit == 0 || limit > MAX_ADMISSION_REVERIFICATION_CANDIDATES {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission reverification candidate limit is outside the supported range".into(),
            ));
        }
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        let mut candidates = Vec::new();
        let mut scanned: u32 = 0;
        let mut cursor: Option<Uuid> = None;
        while candidates.len() < usize::try_from(limit).unwrap_or(usize::MAX)
            && scanned < MAX_ADMISSION_REVERIFICATION_SCAN_ROWS
        {
            let remaining_scan =
                (MAX_ADMISSION_REVERIFICATION_SCAN_ROWS - scanned).min(512);
            let (sql, values) = match cursor {
                None => (
                    format!(
                        "SELECT admission.installation_id AS installation_id, \
                         admission.revision AS revision, \
                         CAST(admission.verification_evidence AS TEXT) AS verification_evidence, \
                         installation.scope_kind AS scope_kind, \
                         installation.tenant_id AS tenant_id \
                         FROM module_artifact_admissions admission \
                         JOIN module_artifact_installations installation \
                           ON installation.installation_id = admission.installation_id \
                         WHERE admission.status IN ('admitted', 'installed', 'inactive', 'failed') \
                         ORDER BY admission.installation_id \
                         LIMIT {}",
                        backend.placeholder(1),
                    ),
                    vec![i64::from(remaining_scan).into()],
                ),
                Some(cursor_id) => (
                    format!(
                        "SELECT admission.installation_id AS installation_id, \
                         admission.revision AS revision, \
                         CAST(admission.verification_evidence AS TEXT) AS verification_evidence, \
                         installation.scope_kind AS scope_kind, \
                         installation.tenant_id AS tenant_id \
                         FROM module_artifact_admissions admission \
                         JOIN module_artifact_installations installation \
                           ON installation.installation_id = admission.installation_id \
                         WHERE admission.status IN ('admitted', 'installed', 'inactive', 'failed') \
                           AND admission.installation_id > {} \
                         ORDER BY admission.installation_id \
                         LIMIT {}",
                        backend.placeholder(1),
                        backend.placeholder(2),
                    ),
                    vec![
                        backend.uuid_value(cursor_id),
                        i64::from(remaining_scan).into(),
                    ],
                ),
            };
            let rows = transaction
                .query_all_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    sql,
                    values,
                ))
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            if rows.is_empty() {
                break;
            }
            let page_len = u32::try_from(rows.len()).unwrap_or(u32::MAX);
            for row in rows {
                scanned = scanned.saturating_add(1);
                let installation_id =
                    required_uuid_from_row(&row, "installation_id", backend)?;
                cursor = Some(installation_id);
                let revision: i64 = row
                    .try_get("", "revision")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                let expected_revision = positive_revision(revision, "admission")?;
                let scope_kind: String = row
                    .try_get("", "scope_kind")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                let stored_tenant_id = optional_uuid_from_row(&row, "tenant_id", backend)?;
                let scope = match (scope_kind.as_str(), stored_tenant_id) {
                    ("platform", None) => ModuleInstallationScope::Platform,
                    ("tenant", Some(tenant_id)) if !tenant_id.is_nil() => {
                        ModuleInstallationScope::Tenant { tenant_id }
                    }
                    _ => {
                        return Err(ModuleInstallationError::Store(format!(
                            "installation {installation_id} carries an invalid persisted scope"
                        )));
                    }
                };
                let evidence_json: String = row
                    .try_get("", "verification_evidence")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                let is_stale =
                    match serde_json::from_str::<ArtifactVerificationEvidence>(&evidence_json) {
                        Ok(evidence) => {
                            evidence.trust_policy_revision
                                != trust_policy.trust_policy_revision
                                || evidence.capability_policy_revision
                                    != trust_policy.capability_policy_revision
                        }
                        Err(_) => true,
                    };
                if is_stale {
                    candidates.push(ArtifactAdmissionReverificationCandidate {
                        installation_id,
                        scope,
                        expected_revision,
                    });
                    if candidates.len() >= usize::try_from(limit).unwrap_or(usize::MAX) {
                        break;
                    }
                }
            }
            if page_len < remaining_scan {
                break;
            }
        }
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(candidates)
    }
}

impl SeaOrmArtifactInstallationStore {
    /// Batched form of the active-installation resolver for one tenant:
    /// one transaction and RLS scope for the whole release set, chunked
    /// `OR` predicates (100 releases per query) instead of one
    /// transaction per artifact. Selection and validation are identical
    /// to the single-release resolver — tenant scope shadows platform,
    /// an explicit tenant disable suppresses the platform candidate,
    /// and the chosen row goes through the same
    /// [`decode_installed_artifact_row`] checks. Storage failures fail
    /// the whole batch; per-release results preserve the caller's order
    /// so absence (`Unavailable`) stays a policy fact for that release
    /// only while a corrupt selected row stays an invariant error.
    pub(crate) async fn resolve_active_many(
        &self,
        releases: &[ArtifactReleaseRef],
        tenant_id: Uuid,
    ) -> Result<
        Vec<Result<InstalledModuleArtifact, ArtifactInstallationResolveError>>,
        ArtifactInstallationResolveError,
    > {
        if releases.is_empty() {
            return Ok(Vec::new());
        }
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &ModuleInstallationScope::Tenant { tenant_id })
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let platform_enabled = match backend {
            SupportedBackend::Postgres => "COALESCE(tenant_lifecycle.enabled, TRUE) = TRUE",
            SupportedBackend::Sqlite => "COALESCE(tenant_lifecycle.enabled, 1) = 1",
        };
        let mut rows = Vec::new();
        for chunk in releases.chunks(100) {
            let mut values = vec![backend.uuid_value(tenant_id)];
            let mut conditions = Vec::with_capacity(chunk.len());
            for (index, release) in chunk.iter().enumerate() {
                let first = 2 + index * 3;
                conditions.push(format!(
                    "(installation.slug = {} AND installation.version = {} \
                      AND installation.payload_digest = {})",
                    backend.placeholder(first),
                    backend.placeholder(first + 1),
                    backend.placeholder(first + 2),
                ));
                values.push(release.slug.clone().into());
                values.push(release.version.clone().into());
                values.push(release.digest.clone().into());
            }
            let release_conditions = conditions.join(" OR ");
            let chunk_rows = transaction
                .query_all_raw(Statement::from_sql_and_values(
                    backend.db_backend(),
                    format!(
                        "SELECT installation.installation_id, installation.data_owner_id, \
                         installation.namespace_instance_id, installation.secret_instance_id, \
                         installation.settings_instance_id, installation.scope_kind, installation.tenant_id, \
                         installation.registry, installation.repository, installation.manifest_digest, \
                         installation.slug, installation.version, installation.payload_digest, \
                         admission.media_type AS payload_media_type, \
                         CAST(installation.descriptor AS TEXT) AS descriptor, \
                         installation.dependency_graph_revision, installation.dependency_graph_digest, \
                         CAST(installation.dependency_lock AS TEXT) AS dependency_lock, \
                         installation.capability_grant_revision, installation.installed_at \
                         FROM module_artifact_installations installation \
                         JOIN module_artifact_admissions admission \
                           ON admission.installation_id = installation.installation_id \
                         LEFT JOIN module_artifact_tenant_lifecycle tenant_lifecycle \
                           ON tenant_lifecycle.installation_id = installation.installation_id \
                          AND tenant_lifecycle.tenant_id = {tenant} \
                         WHERE ({release_conditions}) \
                           AND admission.status = 'active' \
                           AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                           WHERE uninstall.installation_id = installation.installation_id) \
                           AND {platform_enabled} \
                           AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {tenant}) \
                                OR (installation.scope_kind = 'platform' \
                                    AND installation.tenant_id IS NULL)) \
                         ORDER BY installation.slug, installation.version, installation.payload_digest, \
                                  CASE WHEN installation.scope_kind = 'tenant' THEN 0 ELSE 1 END, \
                                  installation.installation_id",
                        tenant = backend.placeholder(1),
                    ),
                    values,
                ))
                .await
                .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
            rows.extend(chunk_rows);
        }
        transaction
            .commit()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;

        let row_keys = rows
            .iter()
            .map(|row| {
                Ok((
                    row.try_get::<String>("", "slug").map_err(|error| {
                        ArtifactInstallationResolveError::Invariant(error.to_string())
                    })?,
                    row.try_get::<String>("", "version").map_err(|error| {
                        ArtifactInstallationResolveError::Invariant(error.to_string())
                    })?,
                    row.try_get::<String>("", "payload_digest").map_err(|error| {
                        ArtifactInstallationResolveError::Invariant(error.to_string())
                    })?,
                ))
            })
            .collect::<Result<Vec<_>, ArtifactInstallationResolveError>>()?;
        let mut results = Vec::with_capacity(releases.len());
        for release in releases {
            // Rows are ordered tenant-scope before platform-scope per
            // release, so the first key match is the selected candidate.
            let selected = rows.iter().zip(&row_keys).find_map(|(row, key)| {
                (key.0 == release.slug && key.1 == release.version && key.2 == release.digest)
                    .then_some(row)
            });
            results.push(match selected {
                Some(row) => decode_installed_artifact_row(row, backend, release),
                None => Err(ArtifactInstallationResolveError::Unavailable),
            });
        }
        Ok(results)
    }

    /// Loads one exact admitted installation by ID and scope for
    /// pre-activation owner work (executor readiness and governed
    /// transition staging), regardless of whether it is the active
    /// serving selection.
    ///
    /// This is deliberately not a serving resolver: it admits the
    /// activation-eligible durable states (`admitted`, `installed`,
    /// `active`, `inactive`, `rolled_back`) and rejects in-flight or
    /// failed admissions and installations with an uninstall operation.
    /// The returned artifact passes the same full row validation as the
    /// active resolver. Callers must not treat the result as permission
    /// to serve traffic; only activation changes the serving selection.
    pub(crate) async fn load_admitted_installation_exact(
        &self,
        installation_id: Uuid,
        scope: &ModuleInstallationScope,
    ) -> Result<InstalledModuleArtifact, ArtifactInstallationResolveError> {
        if installation_id.is_nil() {
            return Err(ArtifactInstallationResolveError::InvalidRequest(
                "exact admitted installation load requires a non-nil installation id".to_string(),
            ));
        }
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, scope)
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let mut values = vec![
            backend.uuid_value(installation_id),
            match scope {
                ModuleInstallationScope::Platform => "platform".into(),
                ModuleInstallationScope::Tenant { .. } => "tenant".into(),
            },
        ];
        let scope_predicate = match scope {
            ModuleInstallationScope::Platform => "installation.tenant_id IS NULL".to_string(),
            ModuleInstallationScope::Tenant { tenant_id } => {
                values.push(backend.uuid_value(*tenant_id));
                format!("installation.tenant_id = {}", backend.placeholder(3))
            }
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.installation_id, installation.data_owner_id, \
                     installation.namespace_instance_id, installation.secret_instance_id, \
                     installation.settings_instance_id, installation.scope_kind, installation.tenant_id, \
                     installation.registry, installation.repository, installation.manifest_digest, \
                     installation.slug, installation.version, installation.payload_digest, \
                     admission.media_type AS payload_media_type, \
                     CAST(installation.descriptor AS TEXT) AS descriptor, \
                     installation.dependency_graph_revision, installation.dependency_graph_digest, \
                     CAST(installation.dependency_lock AS TEXT) AS dependency_lock, \
                     installation.capability_grant_revision, installation.installed_at \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     WHERE installation.installation_id = {} \
                       AND installation.scope_kind = {} \
                       AND {scope_predicate} \
                       AND admission.status IN ('admitted', 'installed', 'active', 'inactive', 'rolled_back') \
                       AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                       WHERE uninstall.installation_id = installation.installation_id) \
                     LIMIT 1",
                    backend.placeholder(1),
                    backend.placeholder(2),
                ),
                values,
            ))
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
            return Err(ArtifactInstallationResolveError::Unavailable);
        };
        let release = ArtifactReleaseRef {
            slug: row
                .try_get("", "slug")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
            version: row
                .try_get("", "version")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
            digest: row
                .try_get("", "payload_digest")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
        };
        transaction
            .commit()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let artifact = decode_installed_artifact_row(&row, backend, &release)?;
        if artifact.scope != *scope {
            return Err(ArtifactInstallationResolveError::Invariant(
                "artifact installation scope does not match the requested exact scope".to_string(),
            ));
        }
        Ok(artifact)
    }

    /// Loads the sole active serving installation for one scope and slug,
    /// for governed-transition predecessor discovery. Returns `None` for
    /// an add-to-absent baseline; ambiguity or a corrupt active row is an
    /// invariant failure, never a silent absent baseline.
    pub(crate) async fn load_active_installation_for_scope(
        &self,
        scope: &ModuleInstallationScope,
        module_slug: &str,
    ) -> Result<Option<InstalledModuleArtifact>, ArtifactInstallationResolveError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, scope)
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let installation_id =
            active_predecessor_installation(&transaction, scope, module_slug, backend)
                .await
                .map_err(|error| match error {
                    ModuleInstallationError::Store(message) => {
                        ArtifactInstallationResolveError::Store(message)
                    }
                    error => ArtifactInstallationResolveError::Invariant(error.to_string()),
                })?;
        transaction
            .commit()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        match installation_id {
            Some(installation_id) => Ok(Some(
                self.load_admitted_installation_exact(installation_id, scope)
                    .await?,
            )),
            None => Ok(None),
        }
    }

    /// Loads a committed activation receipt by operation identity within
    /// one exact scope. Governed-transition resume uses this after the
    /// checkpoint has already reached `Observing`, when the activation
    /// transaction committed but the caller never observed its result.
    pub(crate) async fn load_activation_result(
        &self,
        operation_id: Uuid,
        scope: &ModuleInstallationScope,
    ) -> Result<Option<ArtifactActivationResult>, ArtifactInstallationResolveError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, scope)
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let (scope_kind, tenant_id) = match scope {
            ModuleInstallationScope::Platform => ("platform", None),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
        };
        let scope_predicate = match backend {
            SupportedBackend::Postgres => {
                "installation.scope_kind = $2 AND installation.tenant_id IS NOT DISTINCT FROM $3"
            }
            SupportedBackend::Sqlite => {
                "installation.scope_kind = ?2 AND installation.tenant_id IS ?3"
            }
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT operation.operation_id, operation.predecessor_installation_id, \
                     operation.installation_revision, operation.predecessor_revision \
                     FROM module_artifact_activation_operations operation \
                     JOIN module_artifact_installations installation \
                       ON installation.installation_id = operation.installation_id \
                     WHERE operation.operation_id = {} AND {scope_predicate}",
                    backend.placeholder(1),
                ),
                vec![
                    backend.uuid_value(operation_id),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let invariant = |error: ModuleInstallationError| {
            ArtifactInstallationResolveError::Invariant(error.to_string())
        };
        let installation_revision: i64 = row
            .try_get("", "installation_revision")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let predecessor_revision: Option<i64> = row
            .try_get("", "predecessor_revision")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        Ok(Some(ArtifactActivationResult {
            operation_id: required_uuid_from_row(&row, "operation_id", backend)
                .map_err(invariant)?,
            predecessor_installation_id: optional_uuid_from_row(
                &row,
                "predecessor_installation_id",
                backend,
            )
            .map_err(invariant)?,
            installation_revision: positive_revision(installation_revision, "activation")
                .map_err(invariant)?,
            predecessor_revision: predecessor_revision
                .map(|revision| positive_revision(revision, "activation predecessor"))
                .transpose()
                .map_err(invariant)?,
        }))
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
    ) -> Result<InstalledModuleArtifact, ArtifactInstallationResolveError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &ModuleInstallationScope::Tenant { tenant_id })
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let placeholders = match backend {
            SupportedBackend::Postgres => ("$1", "$2", "$3", "$4"),
            SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4"),
        };
        let platform_enabled = match backend {
            SupportedBackend::Postgres => "COALESCE(tenant_lifecycle.enabled, TRUE) = TRUE",
            SupportedBackend::Sqlite => "COALESCE(tenant_lifecycle.enabled, 1) = 1",
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT installation.installation_id, installation.data_owner_id, \
                     installation.namespace_instance_id, installation.secret_instance_id, \
                     installation.settings_instance_id, installation.scope_kind, installation.tenant_id, \
                     installation.registry, installation.repository, installation.manifest_digest, \
                     installation.slug, installation.version, installation.payload_digest, \
                     admission.media_type AS payload_media_type, \
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
                    backend.uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        let Some(row) = row else {
            transaction
                .commit()
                .await
                .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
            return Err(ArtifactInstallationResolveError::Unavailable);
        };
        transaction
            .commit()
            .await
            .map_err(|error| ArtifactInstallationResolveError::Store(error.to_string()))?;
        decode_installed_artifact_row(&row, backend, release)
    }
}

/// Decodes and fully validates one selected installation row. Shared by
/// the single-release resolver and the batched multi-release resolver so
/// both paths apply identical descriptor, media-type, dependency-lock,
/// and persisted-identity checks; the batched path only changes how rows
/// are fetched, never what a valid row is.
fn decode_installed_artifact_row(
    row: &sea_orm::QueryResult,
    backend: SupportedBackend,
    release: &ArtifactReleaseRef,
) -> Result<InstalledModuleArtifact, ArtifactInstallationResolveError> {
        let installation_id = required_uuid_from_row(row, "installation_id", backend)
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let data_owner_id = required_uuid_from_row(row, "data_owner_id", backend)
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let namespace_instance_id = optional_uuid_from_row(row, "namespace_instance_id", backend)
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let secret_instance_id = optional_uuid_from_row(row, "secret_instance_id", backend)
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let settings_instance_id = required_uuid_from_row(row, "settings_instance_id", backend)
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let scope_kind: String = row
            .try_get("", "scope_kind")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let scope = match scope_kind.as_str() {
            "platform" => ModuleInstallationScope::Platform,
            "tenant" => ModuleInstallationScope::Tenant {
                tenant_id: required_uuid_from_row(row, "tenant_id", backend).map_err(|error| {
                    ArtifactInstallationResolveError::Invariant(error.to_string())
                })?,
            },
            _ => {
                return Err(ArtifactInstallationResolveError::Invariant(
                    "artifact installation has an invalid scope".to_string(),
                ));
            }
        };
        let reference = OciArtifactReference {
            registry: row
                .try_get("", "registry")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
            repository: row
                .try_get("", "repository")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
            digest: row
                .try_get("", "manifest_digest")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
        };
        reference
            .validate()
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
            &row.try_get::<String>("", "descriptor")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
        )
        .map_err(|_| {
            ArtifactInstallationResolveError::Invariant(
                "artifact installation descriptor is invalid".to_string(),
            )
        })?;
        descriptor.validate().map_err(|_| {
            ArtifactInstallationResolveError::Invariant(
                "artifact installation descriptor is invalid".to_string(),
            )
        })?;
        let payload_media_type: String = row
            .try_get("", "payload_media_type")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        if !valid_media_type_for(descriptor.payload_kind, &payload_media_type) {
            return Err(ArtifactInstallationResolveError::Invariant(
                "artifact installation payload media type is invalid".to_string(),
            ));
        }
        let dependency_lock: ModuleDependencyLockGraph = serde_json::from_str(
            &row.try_get::<String>("", "dependency_lock")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
        )
        .map_err(|_| {
            ArtifactInstallationResolveError::Invariant(
                "artifact installation dependency lock is invalid".to_string(),
            )
        })?;
        dependency_lock.validate().map_err(|_| {
            ArtifactInstallationResolveError::Invariant(
                "artifact installation dependency lock is invalid".to_string(),
            )
        })?;
        let persisted_graph_revision: i64 = row
            .try_get("", "dependency_graph_revision")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let persisted_graph_digest: String = row
            .try_get("", "dependency_graph_digest")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let capability_grant_revision: i64 = row
            .try_get("", "capability_grant_revision")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let installed_at = match backend {
            SupportedBackend::Postgres => row
                .try_get::<DateTime<Utc>>("", "installed_at")
                .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?,
            SupportedBackend::Sqlite => {
                let installed_at_text: String =
                    row.try_get("", "installed_at").map_err(|error| {
                        ArtifactInstallationResolveError::Invariant(error.to_string())
                    })?;
                crate::backend::parse_sqlite_datetime(&installed_at_text).map_err(|_| {
                    ArtifactInstallationResolveError::Invariant(
                        "artifact installation timestamp is invalid".to_string(),
                    )
                })?
            }
        };
        let persisted_slug: String = row
            .try_get("", "slug")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let persisted_version: String = row
            .try_get("", "version")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let persisted_payload_digest: String = row
            .try_get("", "payload_digest")
            .map_err(|error| ArtifactInstallationResolveError::Invariant(error.to_string()))?;
        let persisted_graph_revision = u64::try_from(persisted_graph_revision).map_err(|_| {
            ArtifactInstallationResolveError::Invariant(
                "artifact installation dependency graph revision is invalid".to_string(),
            )
        })?;
        if descriptor.slug != persisted_slug
            || descriptor.version != persisted_version
            || descriptor.artifact_digest != persisted_payload_digest
            || descriptor.release_ref() != *release
            || dependency_lock.graph_revision != persisted_graph_revision
            || dependency_lock.graph_digest != persisted_graph_digest
        {
            return Err(ArtifactInstallationResolveError::Invariant(
                "artifact installation immutable state does not match its persisted identity"
                    .to_string(),
            ));
        }
        let capability_grant_revision = u64::try_from(capability_grant_revision)
            .ok()
            .filter(|revision| *revision > 0)
            .ok_or_else(|| {
                ArtifactInstallationResolveError::Invariant(
                    "artifact installation capability revision is invalid".to_string(),
                )
            })?;
        Ok(InstalledModuleArtifact {
            installation_id,
            data_owner_id,
            namespace_instance_id,
            secret_instance_id,
            settings_instance_id,
            scope,
            reference,
            release: release.clone(),
            descriptor,
            payload_media_type,
            dependency_lock,
            capability_grant_revision,
            installed_at,
        })
}

#[async_trait]
impl ArtifactSandboxPolicyResolver for SeaOrmArtifactSandboxPolicyResolver {
    async fn resolve(
        &self,
        artifact: &InstalledModuleArtifact,
        tenant_id: Uuid,
    ) -> Result<SandboxPolicy, ArtifactSandboxPolicyResolveError> {
        if artifact.installation_id.is_nil() || tenant_id.is_nil() {
            return Err(ArtifactSandboxPolicyResolveError::InvalidRequest(
                "artifact sandbox policy requires non-nil identities".to_string(),
            ));
        }
        if matches!(&artifact.scope, ModuleInstallationScope::Tenant { tenant_id: scope_tenant } if *scope_tenant != tenant_id)
        {
            return Err(ArtifactSandboxPolicyResolveError::InvalidRequest(
                "tenant-scoped artifact cannot execute for another tenant".to_string(),
            ));
        }

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ArtifactSandboxPolicyResolveError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &ModuleInstallationScope::Tenant { tenant_id })
            .await
            .map_err(|error| ArtifactSandboxPolicyResolveError::Store(error.to_string()))?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ArtifactSandboxPolicyResolveError::Store(error.to_string()))?;
        let placeholders = match backend {
            SupportedBackend::Postgres => ("$1", "$2", "$3", "$4", "$5"),
            SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4", "?5"),
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
            SupportedBackend::Postgres => "COALESCE(lifecycle.enabled, TRUE) = TRUE",
            SupportedBackend::Sqlite => "COALESCE(lifecycle.enabled, 1) = 1",
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
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
                    backend.uuid_value(artifact.installation_id),
                    backend.uuid_value(tenant_id),
                    artifact.release.slug.clone().into(),
                    artifact.release.version.clone().into(),
                    artifact.release.digest.clone().into(),
                ],
            ))
            .await
            .map_err(|error| ArtifactSandboxPolicyResolveError::Store(error.to_string()))?;
        transaction
            .commit()
            .await
            .map_err(|error| ArtifactSandboxPolicyResolveError::Store(error.to_string()))?;
        let row = row.ok_or(ArtifactSandboxPolicyResolveError::Unavailable)?;
        let revision: i64 = row
            .try_get("", "capability_grant_revision")
            .map_err(|error| ArtifactSandboxPolicyResolveError::Invariant(error.to_string()))?;
        let revision = u64::try_from(revision).map_err(|_| {
            ArtifactSandboxPolicyResolveError::Invariant(
                "sandbox policy revision is outside the supported range".to_string(),
            )
        })?;
        if revision != artifact.capability_grant_revision {
            return Err(ArtifactSandboxPolicyResolveError::RevisionMismatch);
        }
        let policy: SandboxPolicy =
            serde_json::from_str(&row.try_get::<String>("", "policy").map_err(|error| {
                ArtifactSandboxPolicyResolveError::Invariant(error.to_string())
            })?)
            .map_err(|_| {
                ArtifactSandboxPolicyResolveError::Invariant(
                    "sandbox policy is invalid".to_string(),
                )
            })?;
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
    backend: SupportedBackend,
) -> (String, Vec<SqlValue>) {
    let (scope_kind_placeholder, tenant_placeholder) = match backend {
        SupportedBackend::Postgres => ("$7", "$8"),
        SupportedBackend::Sqlite => ("?7", "?8"),
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
            vec!["tenant".into(), backend.uuid_value(*tenant_id)],
        ),
    }
}

struct StoredAdmissionPermissionFacts {
    descriptor: ModuleArtifactDescriptor,
    registry: String,
    repository: String,
    manifest_digest: String,
    permission_registration: ArtifactPermissionRegistrationStatus,
}

async fn stored_admission_permission_facts<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    installation_id: Uuid,
    scope: &ModuleInstallationScope,
) -> Result<Option<StoredAdmissionPermissionFacts>, ModuleInstallationError> {
    let mut values = vec![
        backend.uuid_value(installation_id),
        match scope {
            ModuleInstallationScope::Platform => "platform".into(),
            ModuleInstallationScope::Tenant { .. } => "tenant".into(),
        },
    ];
    let scope_predicate = match scope {
        ModuleInstallationScope::Platform => "installation.tenant_id IS NULL".to_string(),
        ModuleInstallationScope::Tenant { tenant_id } => {
            values.push(backend.uuid_value(*tenant_id));
            format!("installation.tenant_id = {}", backend.placeholder(3))
        }
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT installation.registry, installation.repository, \
                        installation.manifest_digest, \
                        CAST(installation.descriptor AS TEXT) AS descriptor, \
                        admission.permission_registration_status \
                 FROM module_artifact_installations installation \
                 JOIN module_artifact_admissions admission \
                   ON admission.installation_id = installation.installation_id \
                 WHERE installation.installation_id = {} \
                   AND installation.scope_kind = {} \
                   AND {scope_predicate}",
                backend.placeholder(1),
                backend.placeholder(2),
            ),
            values,
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let Some(row) = row else {
        return Ok(None);
    };
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &row.try_get::<String>("", "descriptor")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
    )
    .map_err(|_| {
        ModuleInstallationError::Store("stored admitted artifact descriptor is invalid".into())
    })?;
    descriptor.validate().map_err(|_| {
        ModuleInstallationError::Store("stored admitted artifact descriptor is invalid".into())
    })?;
    let permission_registration = ArtifactPermissionRegistrationStatus::from_store_str(
        &row.try_get::<String>("", "permission_registration_status")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
    )?;
    Ok(Some(StoredAdmissionPermissionFacts {
        descriptor,
        registry: row
            .try_get("", "registry")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        repository: row
            .try_get("", "repository")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        manifest_digest: row
            .try_get("", "manifest_digest")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        permission_registration,
    }))
}

#[async_trait]
impl ArtifactAdmissionStore for SeaOrmArtifactInstallationStore {
    async fn find_admission_replay(
        &self,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<Option<ArtifactAdmissionReplay>, ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, &command.scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let (scope_kind, scope_tenant_key) = admission_command_scope(command);
        let placeholders = match backend {
            SupportedBackend::Postgres => ("$1", "$2", "$3", "$4"),
            SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4"),
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
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
                    backend.uuid_value(command.context.actor_id),
                    backend.uuid_value(command.context.idempotency_key),
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
        let facts = stored_admission_permission_facts(
            &transaction,
            backend,
            installation_id,
            &command.scope,
        )
        .await?
        .ok_or_else(|| {
            ModuleInstallationError::Store(
                "committed admission command is missing its scoped installation".into(),
            )
        })?;
        if facts.registry != command.reference.registry
            || facts.repository != command.reference.repository
            || facts.manifest_digest != command.reference.digest
        {
            return Err(ModuleInstallationError::Store(
                "committed admission command is bound to a different artifact reference".into(),
            ));
        }
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(Some(ArtifactAdmissionReplay {
            result: ArtifactAdmissionResult {
                installation_id,
                created: false,
                permission_registration: facts.permission_registration,
            },
            descriptor: facts.descriptor,
        }))
    }

    async fn mark_permission_registration_complete(
        &self,
        installation_id: Uuid,
        scope: &ModuleInstallationScope,
    ) -> Result<(), ModuleInstallationError> {
        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        configure_rls_scope(&transaction, scope).await?;
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let facts = stored_admission_permission_facts(
            &transaction,
            backend,
            installation_id,
            scope,
        )
        .await?
        .ok_or_else(|| {
            ModuleInstallationError::Store(
                "permission registration completion is missing its scoped installation".into(),
            )
        })?;
        match facts.permission_registration {
            ArtifactPermissionRegistrationStatus::Registered => {}
            ArtifactPermissionRegistrationStatus::NotRequired => {
                return Err(ModuleInstallationError::Store(
                    "permission registration completion was requested for an artifact without permissions"
                        .into(),
                ));
            }
            ArtifactPermissionRegistrationStatus::Pending => {
                if facts.descriptor.permissions.is_empty() {
                    return Err(ModuleInstallationError::Store(
                        "stored artifact permission registration is pending without declared permissions"
                            .into(),
                    ));
                }
                let updated = transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend.db_backend(),
                        format!(
                            "UPDATE module_artifact_admissions \
                             SET permission_registration_status = {} \
                             WHERE installation_id = {} \
                               AND permission_registration_status = {}",
                            backend.placeholder(1),
                            backend.placeholder(2),
                            backend.placeholder(3),
                        ),
                        vec![
                            ArtifactPermissionRegistrationStatus::Registered
                                .as_str()
                                .into(),
                            backend.uuid_value(installation_id),
                            ArtifactPermissionRegistrationStatus::Pending.as_str().into(),
                        ],
                    ))
                    .await
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                if updated.rows_affected() != 1 {
                    return Err(ModuleInstallationError::Store(
                        "artifact permission registration state changed before completion"
                            .into(),
                    ));
                }
            }
        }
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))
    }

    async fn reserve_blob_publication(
        &self,
        staged: &StagedArtifactBlob,
        reserved_at: DateTime<Utc>,
    ) -> Result<(), ModuleInstallationError> {
        validate_blob_publication_reservation(staged)?;
        let expires_at = reserved_at
            .checked_add_signed(chrono::Duration::seconds(
                ARTIFACT_BLOB_PUBLICATION_RESERVATION_LEASE_SECONDS,
            ))
            .ok_or_else(|| {
                ModuleInstallationError::Store(
                    "artifact blob publication reservation lease exceeds the supported timestamp range"
                        .into(),
                )
            })?;
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        // Lock the candidate before inserting the reservation. A GC claim
        // takes the same lock before checking reservations, so the two
        // transactions cannot both conclude that the other side is absent.
        if let Some(candidate) =
            load_blob_gc_candidate(&transaction, backend, &staged.digest, true).await?
            && candidate.state == ArtifactBlobGcCandidateState::Deleting
        {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "artifact blob collection is in progress; retry admission after collection so its bytes are republished"
                    .into(),
            ));
        }
        delete_expired_blob_publication_reservations(
            &transaction,
            backend,
            &staged.digest,
            reserved_at,
        )
        .await?;
        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_blob_publication_reservations \
                     (stage_id, digest, reserved_at, expires_at) VALUES ({}, {}, {}, {})",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                ),
                vec![
                    backend.uuid_value(staged.stage_id),
                    staged.digest.clone().into(),
                    backend.datetime_value(reserved_at),
                    backend.datetime_value(expires_at),
                ],
            ))
            .await
            .map_err(|error| {
                if backend.is_unique_violation(&error) {
                    ModuleInstallationError::AdmissionRevisionConflict(
                        "staged artifact blob already has a publication reservation".into(),
                    )
                } else {
                    ModuleInstallationError::Store(error.to_string())
                }
            })?;
        if inserted.rows_affected() != 1 {
            return Err(ModuleInstallationError::Store(format!(
                "artifact blob publication reservation insert affected {} rows",
                inserted.rows_affected()
            )));
        }
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(())
    }

    async fn release_blob_publication_reservation(
        &self,
        staged: &StagedArtifactBlob,
    ) -> Result<bool, ModuleInstallationError> {
        validate_blob_publication_reservation(staged)?;
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        let deleted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "DELETE FROM module_artifact_blob_publication_reservations \
                     WHERE stage_id = {} AND digest = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                ),
                vec![
                    backend.uuid_value(staged.stage_id),
                    staged.digest.clone().into(),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if deleted.rows_affected() > 1 {
            return Err(ModuleInstallationError::Store(
                "artifact blob publication reservation release affected multiple rows".into(),
            ));
        }
        let released = deleted.rows_affected() == 1;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(released)
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
        let backend = SupportedBackend::new(transaction.get_database_backend())
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let committed_at = self.infrastructure.now();
        let (scope_kind, scope_tenant_key) = admission_command_scope(command);
        let reservation = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                admission_command_insert_sql(backend),
                vec![
                    scope_kind.into(),
                    scope_tenant_key.clone().into(),
                    backend.uuid_value(command.context.actor_id),
                    backend.uuid_value(command.context.idempotency_key),
                    command.context.trace_id.clone().into(),
                    backend.uuid_value(command.context.correlation_id),
                    request_digest.into(),
                    backend.datetime_value(committed_at),
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
                command.context.actor_id,
                command.context.idempotency_key,
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
            let facts = stored_admission_permission_facts(
                &transaction,
                backend,
                installation_id,
                &command.scope,
            )
            .await?
            .ok_or_else(|| {
                ModuleInstallationError::Store(
                    "committed admission command is missing its scoped installation".into(),
                )
            })?;
            fence_blob_gc_candidate_for_admission(
                &transaction,
                backend,
                staged,
                committed_at,
            )
            .await?;
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(ArtifactAdmissionResult {
                installation_id,
                created: false,
                permission_registration: facts.permission_registration,
            });
        }
        fence_blob_gc_candidate_for_admission(&transaction, backend, staged, committed_at)
            .await?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                installation_insert_sql(backend),
                installation_values(artifact, None, backend)?,
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        // Project the descriptor's collision-checked UI resources in the
        // same transaction, so activation never has to deserialize every
        // active descriptor to detect a route/slot conflict.
        insert_artifact_ui_resources(&transaction, backend, artifact).await?;
        // Every installation is born at work generation 1. The durable
        // generation row is the fence delivery admission checks: uninstall
        // advances and retires it, after which queued or delayed work for
        // the installation is never admitted again.
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                work_generation_insert_sql(backend),
                vec![
                    backend.uuid_value(artifact.installation_id),
                    backend.datetime_value(committed_at),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                sandbox_policy_insert_sql(backend),
                sandbox_policy_values(artifact, &command.sandbox_policy, backend, &committed_at)?,
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let bound = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                admission_command_bind_sql(backend),
                vec![
                    backend.uuid_value(artifact.installation_id),
                    scope_kind.into(),
                    scope_tenant_key.into(),
                    backend.uuid_value(command.context.actor_id),
                    backend.uuid_value(command.context.idempotency_key),
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
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                admission_insert_sql(backend),
                admission_values(artifact, staged, evidence, backend, &committed_at)?,
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &command.context,
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
            permission_registration: initial_permission_registration_status(
                &artifact.descriptor,
            ),
        })
    }

    async fn referenced_blob_digests(&self) -> Result<BTreeSet<String>, ModuleInstallationError> {
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        let rows = transaction
            .query_all_raw(Statement::from_string(
                backend.db_backend(),
                "SELECT admission.payload_digest FROM module_artifact_admissions admission \
                 WHERE NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                 WHERE uninstall.installation_id = admission.installation_id)"
                    .to_string(),
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let referenced = rows
            .into_iter()
            .map(|row| {
                row.try_get("", "payload_digest")
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))
            })
            .collect::<Result<BTreeSet<String>, ModuleInstallationError>>()?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(referenced)
    }
}

#[async_trait]
impl ArtifactBlobGcCandidateStore for SeaOrmArtifactInstallationStore {
    async fn mark_unreferenced_blob(
        &self,
        digest: &str,
        first_seen_unreferenced_at: DateTime<Utc>,
        grace_ends_at: DateTime<Utc>,
    ) -> Result<ArtifactBlobGcMarkOutcome, ModuleInstallationError> {
        validate_blob_gc_digest(digest)?;
        if grace_ends_at < first_seen_unreferenced_at {
            return Err(ModuleInstallationError::Store(
                "artifact blob GC grace period cannot end before the first unreferenced observation"
                    .into(),
            ));
        }
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        delete_expired_blob_publication_reservations(
            &transaction,
            backend,
            digest,
            first_seen_unreferenced_at,
        )
        .await?;
        if referenced_blob_exists(&transaction, backend, digest).await?
            || live_blob_publication_reservation_exists(
                &transaction,
                backend,
                digest,
                first_seen_unreferenced_at,
            )
            .await?
        {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(ArtifactBlobGcMarkOutcome::Referenced);
        }
        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_blob_gc_candidates \
                     (digest, first_seen_unreferenced_at, grace_ends_at, state, revision, \
                      delete_started_at, created_at, updated_at) \
                     VALUES ({}, {}, {}, 'marked', 1, NULL, {}, {}) \
                     ON CONFLICT (digest) DO NOTHING",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                ),
                vec![
                    digest.into(),
                    backend.datetime_value(first_seen_unreferenced_at),
                    backend.datetime_value(grace_ends_at),
                    backend.datetime_value(first_seen_unreferenced_at),
                    backend.datetime_value(first_seen_unreferenced_at),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let candidate = load_blob_gc_candidate(&transaction, backend, digest, false)
            .await?
            .ok_or_else(|| {
                ModuleInstallationError::Store(
                    "artifact blob GC candidate disappeared inside its marking transaction"
                        .into(),
                )
            })?;
        let outcome = match inserted.rows_affected() {
            1 => ArtifactBlobGcMarkOutcome::Marked(candidate),
            0 => match candidate.state {
                ArtifactBlobGcCandidateState::Marked => {
                    ArtifactBlobGcMarkOutcome::AlreadyMarked(candidate)
                }
                ArtifactBlobGcCandidateState::Deleting => {
                    ArtifactBlobGcMarkOutcome::AlreadyDeleting(candidate)
                }
            },
            rows => {
                return Err(ModuleInstallationError::Store(format!(
                    "artifact blob GC candidate insert affected {rows} rows"
                )));
            }
        };
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(outcome)
    }

    async fn claim_unreferenced_blob_gc(
        &self,
        digest: &str,
        now: DateTime<Utc>,
    ) -> Result<ArtifactBlobGcClaimOutcome, ModuleInstallationError> {
        validate_blob_gc_digest(digest)?;
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        let Some(candidate) = load_blob_gc_candidate(&transaction, backend, digest, true).await?
        else {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(ArtifactBlobGcClaimOutcome::Missing);
        };

        delete_expired_blob_publication_reservations(&transaction, backend, digest, now)
            .await?;
        if live_blob_publication_reservation_exists(&transaction, backend, digest, now).await? {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(ArtifactBlobGcClaimOutcome::Referenced { revoked: false });
        }

        if referenced_blob_exists(&transaction, backend, digest).await? {
            let revoked = match candidate.state {
                ArtifactBlobGcCandidateState::Marked => {
                    delete_marked_blob_gc_candidate(&transaction, backend, &candidate).await?
                }
                ArtifactBlobGcCandidateState::Deleting => false,
            };
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(ArtifactBlobGcClaimOutcome::Referenced { revoked });
        }

        match candidate.state {
            ArtifactBlobGcCandidateState::Marked if now < candidate.grace_ends_at => {
                transaction
                    .commit()
                    .await
                    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                return Ok(ArtifactBlobGcClaimOutcome::GraceNotElapsed(candidate));
            }
            ArtifactBlobGcCandidateState::Deleting => {
                let delete_started_at = candidate.delete_started_at.ok_or_else(|| {
                    ModuleInstallationError::Store(
                        "deleting artifact blob GC candidate has no claim start timestamp".into(),
                    )
                })?;
                let stale_at = delete_started_at
                    .checked_add_signed(chrono::Duration::seconds(
                        ARTIFACT_BLOB_GC_CLAIM_LEASE_SECONDS,
                    ))
                    .ok_or_else(|| {
                        ModuleInstallationError::Store(
                            "artifact blob GC claim lease exceeds the supported timestamp range"
                                .into(),
                        )
                    })?;
                if now < stale_at {
                    transaction
                        .commit()
                        .await
                        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
                    return Ok(ArtifactBlobGcClaimOutcome::ClaimInProgress(candidate));
                }
            }
            ArtifactBlobGcCandidateState::Marked => {}
        }

        let claimed_revision = candidate.revision.checked_add(1).ok_or_else(|| {
            ModuleInstallationError::Store("artifact blob GC candidate revision overflow".into())
        })?;
        let claimed_revision_value = i64::try_from(claimed_revision).map_err(|_| {
            ModuleInstallationError::Store("artifact blob GC candidate revision overflow".into())
        })?;
        let expected_revision_value = i64::try_from(candidate.revision).map_err(|_| {
            ModuleInstallationError::Store("artifact blob GC candidate revision overflow".into())
        })?;
        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "UPDATE module_artifact_blob_gc_candidates \
                     SET state = 'deleting', revision = {}, delete_started_at = {}, updated_at = {} \
                     WHERE digest = {} AND state = {} AND revision = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                    backend.placeholder(6),
                ),
                vec![
                    claimed_revision_value.into(),
                    backend.datetime_value(now),
                    backend.datetime_value(now),
                    digest.into(),
                    candidate.state.as_str().into(),
                    expected_revision_value.into(),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        if updated.rows_affected() != 1 {
            return Err(ModuleInstallationError::Store(
                "artifact blob GC candidate changed while its collection claim was recorded"
                    .into(),
            ));
        }
        let claimed = ArtifactBlobGcCandidate {
            state: ArtifactBlobGcCandidateState::Deleting,
            revision: claimed_revision,
            delete_started_at: Some(now),
            ..candidate
        };
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(ArtifactBlobGcClaimOutcome::Claimed(claimed))
    }

    async fn revoke_marked_blob_gc_candidate(
        &self,
        digest: &str,
    ) -> Result<bool, ModuleInstallationError> {
        validate_blob_gc_digest(digest)?;
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        let Some(candidate) = load_blob_gc_candidate(&transaction, backend, digest, true).await?
        else {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(false);
        };
        if candidate.state == ArtifactBlobGcCandidateState::Deleting
            || !referenced_blob_exists(&transaction, backend, digest).await?
        {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(false);
        }
        let revoked = delete_marked_blob_gc_candidate(&transaction, backend, &candidate).await?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(revoked)
    }

    async fn blob_gc_candidates(
        &self,
    ) -> Result<Vec<ArtifactBlobGcCandidate>, ModuleInstallationError> {
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        let rows = transaction
            .query_all_raw(Statement::from_string(
                backend.db_backend(),
                "SELECT digest, first_seen_unreferenced_at, grace_ends_at, state, revision, \
                        delete_started_at \
                 FROM module_artifact_blob_gc_candidates ORDER BY digest ASC"
                    .to_string(),
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let candidates = rows
            .into_iter()
            .map(|row| blob_gc_candidate_from_row(&row, backend))
            .collect::<Result<Vec<_>, _>>()?;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(candidates)
    }

    async fn complete_blob_gc(
        &self,
        digest: &str,
        claimed_revision: u64,
    ) -> Result<bool, ModuleInstallationError> {
        validate_blob_gc_digest(digest)?;
        if claimed_revision == 0 {
            return Err(ModuleInstallationError::Store(
                "artifact blob GC completion requires a positive claimed revision".into(),
            ));
        }
        let (transaction, backend) = self.begin_platform_blob_gc_transaction().await?;
        let Some(candidate) = load_blob_gc_candidate(&transaction, backend, digest, true).await?
        else {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(false);
        };
        if candidate.state != ArtifactBlobGcCandidateState::Deleting
            || candidate.revision != claimed_revision
            || referenced_blob_exists(&transaction, backend, digest).await?
            || blob_publication_reservation_exists(&transaction, backend, digest).await?
        {
            transaction
                .commit()
                .await
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
            return Ok(false);
        }
        let claimed_revision_value = i64::try_from(claimed_revision).map_err(|_| {
            ModuleInstallationError::Store("artifact blob GC candidate revision overflow".into())
        })?;
        let deleted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "DELETE FROM module_artifact_blob_gc_candidates \
                     WHERE digest = {} AND state = 'deleting' AND revision = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                ),
                vec![digest.into(), claimed_revision_value.into()],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let completed = deleted.rows_affected() == 1;
        transaction
            .commit()
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(completed)
    }
}

fn validate_blob_gc_digest(digest: &str) -> Result<(), ModuleInstallationError> {
    if digest.is_empty() || digest.trim() != digest {
        return Err(ModuleInstallationError::Store(
            "artifact blob GC requires a non-empty canonical digest".into(),
        ));
    }
    Ok(())
}

async fn referenced_blob_exists<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    digest: &str,
) -> Result<bool, ModuleInstallationError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT 1 AS present FROM module_artifact_admissions admission \
                 WHERE admission.payload_digest = {} \
                   AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                   WHERE uninstall.installation_id = admission.installation_id) \
                 LIMIT 1",
                backend.placeholder(1),
            ),
            vec![digest.into()],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(row.is_some())
}

fn validate_blob_publication_reservation(
    staged: &StagedArtifactBlob,
) -> Result<(), ModuleInstallationError> {
    validate_blob_gc_digest(&staged.digest)?;
    if staged.stage_id.is_nil() {
        return Err(ModuleInstallationError::Store(
            "artifact blob publication reservation requires a non-nil stage identity".into(),
        ));
    }
    Ok(())
}

async fn delete_expired_blob_publication_reservations<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    digest: &str,
    now: DateTime<Utc>,
) -> Result<(), ModuleInstallationError> {
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "DELETE FROM module_artifact_blob_publication_reservations \
                 WHERE digest = {} AND expires_at <= {}",
                backend.placeholder(1),
                backend.placeholder(2),
            ),
            vec![digest.into(), backend.datetime_value(now)],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(())
}

async fn live_blob_publication_reservation_exists<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    digest: &str,
    now: DateTime<Utc>,
) -> Result<bool, ModuleInstallationError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT 1 AS present FROM module_artifact_blob_publication_reservations \
                 WHERE digest = {} AND expires_at > {} LIMIT 1",
                backend.placeholder(1),
                backend.placeholder(2),
            ),
            vec![digest.into(), backend.datetime_value(now)],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(row.is_some())
}

async fn blob_publication_reservation_exists<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    digest: &str,
) -> Result<bool, ModuleInstallationError> {
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT 1 AS present FROM module_artifact_blob_publication_reservations \
                 WHERE digest = {} LIMIT 1",
                backend.placeholder(1),
            ),
            vec![digest.into()],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(row.is_some())
}

async fn consume_blob_publication_reservation_for_admission<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    staged: &StagedArtifactBlob,
    committed_at: DateTime<Utc>,
) -> Result<(), ModuleInstallationError> {
    validate_blob_publication_reservation(staged)?;
    let deleted = connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "DELETE FROM module_artifact_blob_publication_reservations \
                 WHERE stage_id = {} AND digest = {} AND expires_at > {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
            ),
            vec![
                backend.uuid_value(staged.stage_id),
                staged.digest.clone().into(),
                backend.datetime_value(committed_at),
            ],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    if deleted.rows_affected() != 1 {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "artifact blob publication reservation is missing or expired; retry admission so its bytes are republished"
                .into(),
        ));
    }
    Ok(())
}

async fn load_blob_gc_candidate<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    digest: &str,
    for_update: bool,
) -> Result<Option<ArtifactBlobGcCandidate>, ModuleInstallationError> {
    let lock_clause = match (backend, for_update) {
        (SupportedBackend::Postgres, true) => " FOR UPDATE",
        _ => "",
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT digest, first_seen_unreferenced_at, grace_ends_at, state, revision, \
                        delete_started_at \
                 FROM module_artifact_blob_gc_candidates WHERE digest = {}{}",
                backend.placeholder(1),
                lock_clause,
            ),
            vec![digest.into()],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    row.as_ref()
        .map(|row| blob_gc_candidate_from_row(row, backend))
        .transpose()
}

fn blob_gc_candidate_from_row(
    row: &sea_orm::QueryResult,
    backend: SupportedBackend,
) -> Result<ArtifactBlobGcCandidate, ModuleInstallationError> {
    let revision: i64 = row
        .try_get("", "revision")
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let revision = u64::try_from(revision).map_err(|_| {
        ModuleInstallationError::Store(
            "artifact blob GC candidate revision is outside the supported range".into(),
        )
    })?;
    if revision == 0 {
        return Err(ModuleInstallationError::Store(
            "artifact blob GC candidate revision must be positive".into(),
        ));
    }
    let state: String = row
        .try_get("", "state")
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(ArtifactBlobGcCandidate {
        digest: row
            .try_get("", "digest")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        first_seen_unreferenced_at: backend
            .datetime_from_row(row, "first_seen_unreferenced_at")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        grace_ends_at: backend
            .datetime_from_row(row, "grace_ends_at")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        state: ArtifactBlobGcCandidateState::from_stored(&state)?,
        revision,
        delete_started_at: backend
            .optional_datetime_from_row(row, "delete_started_at")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
    })
}

async fn delete_marked_blob_gc_candidate<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    candidate: &ArtifactBlobGcCandidate,
) -> Result<bool, ModuleInstallationError> {
    if candidate.state != ArtifactBlobGcCandidateState::Marked {
        return Ok(false);
    }
    let revision = i64::try_from(candidate.revision).map_err(|_| {
        ModuleInstallationError::Store("artifact blob GC candidate revision overflow".into())
    })?;
    let deleted = connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "DELETE FROM module_artifact_blob_gc_candidates \
                 WHERE digest = {} AND state = 'marked' AND revision = {}",
                backend.placeholder(1),
                backend.placeholder(2),
            ),
            vec![candidate.digest.clone().into(), revision.into()],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    if deleted.rows_affected() > 1 {
        return Err(ModuleInstallationError::Store(
            "artifact blob GC candidate revocation affected multiple rows".into(),
        ));
    }
    Ok(deleted.rows_affected() == 1)
}

async fn fence_blob_gc_candidate_for_admission<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    staged: &StagedArtifactBlob,
    committed_at: DateTime<Utc>,
) -> Result<(), ModuleInstallationError> {
    let candidate =
        load_blob_gc_candidate(connection, backend, &staged.digest, true).await?;
    // The exact reservation proves that no GC claim could be authorized
    // between this attempt's CAS publish and this commit transaction. It is
    // consumed here so the committed admission reference replaces the
    // pending reference atomically.
    consume_blob_publication_reservation_for_admission(
        connection,
        backend,
        staged,
        committed_at,
    )
    .await?;
    let Some(candidate) = candidate else {
        return Ok(());
    };
    match candidate.state {
        ArtifactBlobGcCandidateState::Marked => {
            if delete_marked_blob_gc_candidate(connection, backend, &candidate).await? {
                Ok(())
            } else {
                Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "artifact blob GC candidate changed while the admission reference was fenced"
                        .into(),
                ))
            }
        }
        ArtifactBlobGcCandidateState::Deleting => {
            Err(ModuleInstallationError::AdmissionRevisionConflict(
                "artifact blob collection is in progress; retry admission after collection so its bytes are republished"
                    .into(),
            ))
        }
    }
}

fn admission_command_scope(command: &ArtifactAdmissionCommand) -> (&'static str, String) {
    match &command.scope {
        ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
    }
}

fn admission_command_insert_sql(backend: SupportedBackend) -> String {
    let placeholders = (1..=8)
        .map(|index| backend.placeholder(index))
        .collect::<Vec<_>>();
    format!(
        "INSERT INTO module_artifact_admission_commands (\
            scope_kind, scope_tenant_key, actor_id, idempotency_key, trace_id, correlation_id, request_digest, committed_at\
         ) VALUES ({}) ON CONFLICT DO NOTHING",
        placeholders.join(", "),
    )
}

fn admission_command_bind_sql(backend: SupportedBackend) -> String {
    let placeholders = match backend {
        SupportedBackend::Postgres => ("$1", "$2", "$3", "$4", "$5", "$6"),
        SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4", "?5", "?6"),
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
    backend: SupportedBackend,
    scope_kind: &str,
    scope_tenant_key: &str,
    actor_id: Uuid,
    idempotency_key: Uuid,
) -> Result<Option<(String, Option<Uuid>)>, ModuleInstallationError> {
    let placeholders = match backend {
        SupportedBackend::Postgres => ("$1", "$2", "$3", "$4"),
        SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4"),
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
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
                backend.uuid_value(actor_id),
                backend.uuid_value(idempotency_key),
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
    backend: SupportedBackend,
) -> Result<Option<Uuid>, ModuleInstallationError> {
    optional_uuid_from_row(row, "installation_id", backend)
}

fn optional_uuid_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: SupportedBackend,
) -> Result<Option<Uuid>, ModuleInstallationError> {
    backend
        .optional_uuid_from_row(row, column)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))
}

fn required_uuid_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: SupportedBackend,
) -> Result<Uuid, ModuleInstallationError> {
    backend
        .uuid_from_row(row, column)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))
}

/// Replays a completed tenant-lifecycle command from its immutable receipt.
///
/// The mutable tenant-intent row retains only current state. Replays must not
/// depend on that row because a later enable or disable command may have
/// advanced it before an uncertain client retries its original request.
async fn replay_artifact_tenant_lifecycle_operation<C: ConnectionTrait>(
    connection: &C,
    request: &ArtifactTenantLifecycleCommand,
) -> Result<Option<u64>, ModuleInstallationError> {
    let backend = SupportedBackend::new(connection.get_database_backend())
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let placeholders = match backend {
        SupportedBackend::Postgres => ("$1", "$2"),
        SupportedBackend::Sqlite => ("?1", "?2"),
    };
    let Some(row) = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT installation_id, requested_enabled, expected_revision, \
                        committed_revision, actor_id, trace_id, correlation_id, reason \
                 FROM module_artifact_tenant_lifecycle_operations \
                 WHERE tenant_id = {} AND idempotency_key = {}",
                placeholders.0, placeholders.1,
            ),
            vec![
                backend.uuid_value(request.tenant_id),
                backend.uuid_value(request.context.idempotency_key),
            ],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
    else {
        return Ok(None);
    };

    let installation_id = required_uuid_from_row(&row, "installation_id", backend)?;
    let requested_enabled = match backend {
        SupportedBackend::Postgres => row
            .try_get::<bool>("", "requested_enabled")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
        SupportedBackend::Sqlite => {
            row.try_get::<i64>("", "requested_enabled")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                != 0
        }
    };
    let expected_revision: i64 = row
        .try_get("", "expected_revision")
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    if installation_id != request.installation_id
        || requested_enabled != request.enabled
        || expected_revision != request.expected_revision as i64
    {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "idempotency key was already used for a different tenant lifecycle command".into(),
        ));
    }

    let committed_revision: i64 = row
        .try_get("", "committed_revision")
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let committed_revision = u64::try_from(committed_revision).map_err(|_| {
        ModuleInstallationError::AdmissionRevisionConflict(
            "tenant lifecycle receipt revision is outside the supported range".into(),
        )
    })?;
    if committed_revision == 0 {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "tenant lifecycle receipt revision is invalid".into(),
        ));
    }
    Ok(Some(committed_revision))
}

async fn record_artifact_tenant_lifecycle_operation<C: ConnectionTrait>(
    connection: &C,
    operation_id: Uuid,
    request: &ArtifactTenantLifecycleCommand,
    committed_revision: u64,
) -> Result<(), ModuleInstallationError> {
    let backend = SupportedBackend::new(connection.get_database_backend())
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let values = match backend {
        SupportedBackend::Postgres => "$1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,NOW()",
        SupportedBackend::Sqlite => "?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')",
    };
    let recorded = connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_artifact_tenant_lifecycle_operations \
                 (operation_id, installation_id, tenant_id, requested_enabled, expected_revision, \
                  committed_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) \
                 VALUES ({values})"
            ),
            vec![
                backend.uuid_value(operation_id),
                backend.uuid_value(request.installation_id),
                backend.uuid_value(request.tenant_id),
                request.enabled.into(),
                (request.expected_revision as i64).into(),
                (committed_revision as i64).into(),
                backend.uuid_value(request.context.actor_id),
                request.context.trace_id.clone().into(),
                backend.uuid_value(request.context.correlation_id),
                request.reason.clone().into(),
                backend.uuid_value(request.context.idempotency_key),
            ],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    if recorded.rows_affected() != 1 {
        return Err(ModuleInstallationError::Store(
            "tenant lifecycle operation receipt was not recorded".into(),
        ));
    }
    Ok(())
}

async fn configure_rls_scope<C: ConnectionTrait>(
    connection: &C,
    scope: &ModuleInstallationScope,
) -> Result<(), ModuleInstallationError> {
    // The GUCs themselves are owned by `crate::backend`; this adapter only
    // translates the installation scope and maps the error type.
    let result = match scope {
        // A platform-owned lifecycle transaction must examine every tenant
        // overlay before it can activate a platform contribution. This
        // setting is transaction-local and is accepted only by the
        // module-control-plane RLS policy.
        ModuleInstallationScope::Platform => crate::backend::set_platform_scope(connection).await,
        ModuleInstallationScope::Tenant { tenant_id } => {
            crate::backend::set_tenant_scope(connection, *tenant_id).await
        }
    };
    result.map_err(|error| ModuleInstallationError::Store(error.to_string()))
}

fn installation_insert_sql(backend: SupportedBackend) -> String {
    let placeholders = (1..=23)
        .map(|index| backend.placeholder(index))
        .collect::<Vec<_>>();
    format!(
        "INSERT INTO module_artifact_installations (\
            installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, \
            slug, version, payload_kind, runtime_abi, payload_digest, entrypoint, descriptor, \
            data_owner_id, settings_instance_id, dependency_graph_revision, dependency_graph_digest, dependency_lock, installed_at, \
            previous_installation_id, capability_grant_revision, namespace_instance_id, secret_instance_id\
         ) VALUES ({})",
        placeholders.join(", ")
    )
}

fn work_generation_insert_sql(backend: SupportedBackend) -> String {
    let (installation_id, updated_at, not_retired) = match backend {
        SupportedBackend::Postgres => ("$1", "$2", "FALSE"),
        SupportedBackend::Sqlite => ("?1", "?2", "0"),
    };
    format!(
        "INSERT INTO module_artifact_work_generations (\
            installation_id, work_generation, retired, retired_at, updated_at\
         ) VALUES ({installation_id}, 1, {not_retired}, NULL, {updated_at})"
    )
}

fn work_generation_retire_sql(backend: SupportedBackend) -> String {
    match backend {
        SupportedBackend::Postgres => "INSERT INTO module_artifact_work_generations (installation_id, work_generation, retired, retired_at, updated_at) VALUES ($1, 2, TRUE, NOW(), NOW()) ON CONFLICT (installation_id) DO UPDATE SET work_generation = module_artifact_work_generations.work_generation + 1, retired = TRUE, retired_at = NOW(), updated_at = NOW()".to_string(),
        SupportedBackend::Sqlite => "INSERT INTO module_artifact_work_generations (installation_id, work_generation, retired, retired_at, updated_at) VALUES (?1, 2, 1, strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now'), strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')) ON CONFLICT (installation_id) DO UPDATE SET work_generation = work_generation + 1, retired = 1, retired_at = strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now'), updated_at = strftime('%Y-%m-%dT%H:%M:%f+00:00', 'now')".to_string(),
    }
}

fn sandbox_policy_insert_sql(backend: SupportedBackend) -> String {
    let placeholders = (1..=5)
        .map(|index| backend.placeholder(index))
        .collect::<Vec<_>>();
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
    backend: SupportedBackend,
    committed_at: &DateTime<Utc>,
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
        backend.uuid_value(artifact.installation_id),
        backend.optional_uuid_value(tenant_id),
        revision.into(),
        SqlValue::Json(Some(Box::new(policy))),
        backend.datetime_value(*committed_at),
    ])
}

fn validate_sandbox_policy(
    artifact: &InstalledModuleArtifact,
    policy: &SandboxPolicy,
) -> Result<(), ArtifactSandboxPolicyResolveError> {
    validate_sandbox_policy_for_admission(&artifact.descriptor, policy)
        .map_err(|error| ArtifactSandboxPolicyResolveError::Invariant(error.to_string()))
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
    backend: SupportedBackend,
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
    let installation_id = backend.uuid_value(artifact.installation_id);
    let tenant_id = backend.optional_uuid_value(tenant_id);
    let installed_at = backend.datetime_value(artifact.installed_at);
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
        backend.uuid_value(artifact.data_owner_id),
        backend.uuid_value(artifact.settings_instance_id),
        dependency_graph_revision.into(),
        artifact.dependency_lock.graph_digest.clone().into(),
        SqlValue::Json(Some(Box::new(dependency_lock))),
        installed_at,
        backend.optional_uuid_value(previous_installation_id),
        capability_grant_revision.into(),
        backend.optional_uuid_value(artifact.namespace_instance_id),
        backend.optional_uuid_value(artifact.secret_instance_id),
    ])
}

/// Refuses a plain serving-selection mutation while an unresolved
/// governed transition owns the same slug. Tenant-scope mutations are
/// also fenced by platform-scope transitions for the slug, because a
/// platform selection change alters what every tenant resolves. The
/// governed activation itself does not use this guard: it loads and
/// revalidates its exact checkpoint instead.
async fn assert_no_unresolved_governed_checkpoint<C: ConnectionTrait>(
    connection: &C,
    scope: &ModuleInstallationScope,
    module_slug: &str,
    action: &str,
) -> Result<(), ModuleInstallationError> {
    let fences = match scope {
        ModuleInstallationScope::Tenant { tenant_id } => {
            TransitionCheckpointStore::list_active_checkpoints_for_slug_and_tenant_or_platform(
                connection,
                module_slug,
                *tenant_id,
            )
            .await
        }
        ModuleInstallationScope::Platform => {
            TransitionCheckpointStore::list_active_checkpoints_for_tenant(connection, None)
                .await
                .map(|checkpoints| {
                    checkpoints
                        .into_iter()
                        .filter(|checkpoint| checkpoint.module_slug == module_slug)
                        .collect::<Vec<_>>()
                })
        }
    }
    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    if let Some(checkpoint) = fences.first() {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(format!(
            "plain artifact {action} is fenced by unresolved governed transition `{}`",
            checkpoint.operation_id
        )));
    }
    Ok(())
}

async fn active_predecessor_installation<C: ConnectionTrait>(
    connection: &C,
    scope: &ModuleInstallationScope,
    slug: &str,
    backend: SupportedBackend,
) -> Result<Option<Uuid>, ModuleInstallationError> {
    let (scope_kind, tenant_id) = match scope {
        ModuleInstallationScope::Platform => ("platform", None),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
    };
    let placeholders = match backend {
        SupportedBackend::Postgres => ("$1", "$2", "$3", "$4"),
        SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4"),
    };
    let sql = format!(
        "SELECT installation.installation_id \
         FROM module_artifact_installations installation \
         JOIN module_artifact_admissions admission \
           ON admission.installation_id = installation.installation_id \
         WHERE installation.scope_kind = {} \
           AND ((installation.tenant_id IS NULL AND {} IS NULL) \
                OR installation.tenant_id = {}) \
           AND installation.slug = {} \
           AND admission.status = 'active' \
           AND NOT EXISTS ( \
               SELECT 1 FROM module_artifact_uninstall_operations uninstall \
               WHERE uninstall.installation_id = installation.installation_id \
           ) \
         LIMIT 2",
        placeholders.0, placeholders.1, placeholders.2, placeholders.3,
    );
    let values = vec![
        scope_kind.into(),
        backend.optional_uuid_value(tenant_id),
        backend.optional_uuid_value(tenant_id),
        slug.into(),
    ];
    let rows = connection
        .query_all_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            sql,
            values,
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    match rows.as_slice() {
        [] => Ok(None),
        [row] => match backend {
            SupportedBackend::Postgres => row
                .try_get("", "installation_id")
                .map(Some)
                .map_err(|error| ModuleInstallationError::Store(error.to_string())),
            SupportedBackend::Sqlite => row
                .try_get::<String>("", "installation_id")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?
                .parse::<Uuid>()
                .map(Some)
                .map_err(|error| ModuleInstallationError::Store(error.to_string())),
        },
        _ => Err(ModuleInstallationError::AdmissionRevisionConflict(
            "rollback predecessor is ambiguous because the scope has multiple active installations"
                .into(),
        )),
    }
}

/// Serializes activation for one `(scope, slug)` via an upserted lock row:
/// the row lock taken by the `DO UPDATE` is held until the caller's
/// transaction ends. Lock rows are intentionally permanent tombstones —
/// one small row per `(scope, slug)` ever activated. Deleting a row when a
/// slug's last installation is uninstalled would race a concurrent
/// transaction that is blocked waiting on that same row lock, so the
/// bounded tombstone is the correct lifecycle.
pub(crate) async fn acquire_artifact_activation_lock<C: ConnectionTrait>(
    connection: &C,
    scope: &ModuleInstallationScope,
    slug: &str,
) -> Result<(), ModuleInstallationError> {
    let backend = SupportedBackend::new(connection.get_database_backend())
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let (scope_kind, scope_tenant_key) = match scope {
        ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
    };
    let values = match backend {
        SupportedBackend::Postgres => "$1,$2,$3",
        SupportedBackend::Sqlite => "?1,?2,?3",
    };
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_artifact_activation_locks \
                 (scope_kind, scope_tenant_key, slug) VALUES ({values}) \
                 ON CONFLICT (scope_kind, scope_tenant_key, slug) \
                 DO UPDATE SET slug = EXCLUDED.slug"
            ),
            vec![scope_kind.into(), scope_tenant_key.into(), slug.into()],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(())
}

/// Persists the admission-time UI-resource projection for one immutable
/// installation. Called exactly once per installation, inside the
/// admission transaction that creates its row; the projection is derived
/// data over the immutable descriptor, so it is never updated afterwards.
async fn insert_artifact_ui_resources<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    artifact: &InstalledModuleArtifact,
) -> Result<(), ModuleInstallationError> {
    let (scope_kind, tenant_id) = match &artifact.scope {
        ModuleInstallationScope::Platform => ("platform", None),
        ModuleInstallationScope::Tenant { tenant_id } => ("tenant", Some(*tenant_id)),
    };
    for resource in artifact_ui_resources(&artifact.descriptor) {
        connection
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_ui_resources \
                     (installation_id, resource_kind, resource_key, scope_kind, tenant_id) \
                     VALUES ({}, {}, {}, {}, {})",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                    backend.placeholder(5),
                ),
                vec![
                    backend.uuid_value(artifact.installation_id),
                    resource.kind.as_str().into(),
                    resource.key.into(),
                    scope_kind.into(),
                    backend.optional_uuid_value(tenant_id),
                ],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    }
    Ok(())
}

fn artifact_ui_resources(descriptor: &ModuleArtifactDescriptor) -> BTreeSet<ArtifactUiResource> {
    descriptor
        .ui_contributions
        .iter()
        .filter_map(|contribution| match &contribution.content {
            ArtifactUiContributionContent::Navigation { route, .. } => Some(ArtifactUiResource {
                kind: ArtifactUiResourceKind::NavigationRoute,
                key: route.clone(),
            }),
            ArtifactUiContributionContent::StorefrontSlot { slot, .. } => {
                Some(ArtifactUiResource {
                    kind: ArtifactUiResourceKind::StorefrontSlot,
                    key: slot.clone(),
                })
            }
            _ => None,
        })
        .collect()
}

/// Locks every resource identity in one deterministic global order. A platform
/// activation can affect every tenant overlay, so a scope-local lock would
/// permit a concurrent tenant activation to create a route/slot collision.
/// Lock rows are permanent tombstones (one per resource identity ever
/// declared) for the same reason as `acquire_artifact_activation_lock`.
async fn acquire_artifact_ui_resource_locks<C: ConnectionTrait>(
    connection: &C,
    resources: &BTreeSet<ArtifactUiResource>,
) -> Result<(), ModuleInstallationError> {
    let backend = SupportedBackend::new(connection.get_database_backend())
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let values = match backend {
        SupportedBackend::Postgres => "$1,$2",
        SupportedBackend::Sqlite => "?1,?2",
    };
    for resource in resources {
        connection
            .execute_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "INSERT INTO module_artifact_ui_contribution_locks \
                     (resource_kind, resource_key) \
                     VALUES ({values}) \
                     ON CONFLICT (resource_kind, resource_key) \
                     DO UPDATE SET resource_key = EXCLUDED.resource_key"
                ),
                vec![resource.kind.as_str().into(), resource.key.clone().into()],
            ))
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    }
    Ok(())
}

/// Rejects an activation whose declared navigation route or storefront slot
/// would become visible alongside an active artifact in the same effective
/// tenant surface. The candidate's own slug is intentionally excluded because
/// activation replaces its direct predecessor atomically.
async fn assert_artifact_ui_contributions_available<C: ConnectionTrait>(
    connection: &C,
    scope: &ModuleInstallationScope,
    candidate_slug: &str,
    candidate_descriptor: &ModuleArtifactDescriptor,
) -> Result<(), ModuleInstallationError> {
    let resources = artifact_ui_resources(candidate_descriptor);
    if resources.is_empty() {
        return Ok(());
    }
    acquire_artifact_ui_resource_locks(connection, &resources).await?;

    let backend = SupportedBackend::new(connection.get_database_backend())
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    // Collision detection reads the admission-time UI-resource
    // projection (indexed by resource identity) instead of deserializing
    // every active descriptor on the surface. Descriptor validity is an
    // admission-time invariant, so the read path trusts the projection.
    let mut values: Vec<SqlValue> = vec![candidate_slug.to_string().into()];
    let surface_clause = match scope {
        // A platform activation must not collide with any tenant overlay.
        ModuleInstallationScope::Platform => "1 = 1".to_string(),
        ModuleInstallationScope::Tenant { tenant_id } => {
            values.push(backend.uuid_value(*tenant_id));
            format!(
                "(installation.scope_kind = 'platform' \
                    OR (installation.scope_kind = 'tenant' \
                        AND installation.tenant_id = {}))",
                backend.placeholder(2)
            )
        }
    };
    let mut resource_clauses = Vec::with_capacity(resources.len());
    for resource in &resources {
        let kind_placeholder = backend.placeholder(values.len() + 1);
        values.push(resource.kind.as_str().into());
        let key_placeholder = backend.placeholder(values.len() + 1);
        values.push(resource.key.clone().into());
        resource_clauses.push(format!(
            "(resource.resource_kind = {kind_placeholder} \
              AND resource.resource_key = {key_placeholder})"
        ));
    }
    let sql = format!(
        "SELECT 1 AS collision \
         FROM module_artifact_ui_resources resource \
         JOIN module_artifact_installations installation \
           ON installation.installation_id = resource.installation_id \
         JOIN module_artifact_admissions admission \
           ON admission.installation_id = installation.installation_id \
         WHERE admission.status = 'active' \
           AND installation.slug <> {} \
           AND {surface_clause} \
           AND NOT EXISTS ( \
             SELECT 1 FROM module_artifact_uninstall_operations uninstall \
             WHERE uninstall.installation_id = installation.installation_id \
           ) \
           AND ({}) \
         LIMIT 1",
        backend.placeholder(1),
        resource_clauses.join(" OR "),
    );
    let collision = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            sql,
            values,
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    if collision.is_some() {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "activation conflicts with an active artifact UI contribution".into(),
        ));
    }
    Ok(())
}

fn positive_revision(revision: i64, operation: &str) -> Result<u64, ModuleInstallationError> {
    u64::try_from(revision)
        .ok()
        .filter(|revision| *revision > 0)
        .ok_or_else(|| {
            ModuleInstallationError::Store(format!("{operation} revision is not positive"))
        })
}

fn admission_insert_sql(backend: SupportedBackend) -> String {
    let placeholders = (1..=10)
        .map(|index| backend.placeholder(index))
        .collect::<Vec<_>>();
    format!(
        "INSERT INTO module_artifact_admissions (\
            stage_id, installation_id, payload_digest, media_type, size_bytes, verification_evidence, status, revision, committed_at, permission_registration_status\
         ) VALUES ({})",
        placeholders.join(", ")
    )
}

fn admission_values(
    artifact: &InstalledModuleArtifact,
    staged: &StagedArtifactBlob,
    evidence: &ArtifactVerificationEvidence,
    backend: SupportedBackend,
    committed_at: &DateTime<Utc>,
) -> Result<Vec<SqlValue>, ModuleInstallationError> {
    if evidence.manifest_digest != artifact.reference.digest
        || evidence.payload_digest != staged.digest
        || evidence.media_type != staged.media_type
    {
        return Err(ModuleInstallationError::VerificationMismatch(
            "verification evidence does not match the admitted artifact".into(),
        ));
    }
    let evidence = serde_json::to_value(evidence)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    let size_bytes = i64::try_from(staged.size_bytes).map_err(|_| {
        ModuleInstallationError::Blob("artifact payload exceeds database size range".into())
    })?;
    Ok(vec![
        backend.uuid_value(staged.stage_id),
        backend.uuid_value(artifact.installation_id),
        staged.digest.clone().into(),
        staged.media_type.clone().into(),
        size_bytes.into(),
        SqlValue::Json(Some(Box::new(evidence))),
        ArtifactAdmissionStatus::Admitted.as_str().into(),
        1_i64.into(),
        backend.datetime_value(*committed_at),
        initial_permission_registration_status(&artifact.descriptor)
            .as_str()
            .into(),
    ])
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
    infrastructure: ControlPlaneInfrastructure,
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

    pub async fn find_admission_replay(
        &self,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<Option<ArtifactAdmissionReplay>, ModuleInstallationError> {
        self.store
            .find_admission_replay(command, request_digest)
            .await
    }

    pub async fn mark_permission_registration_complete(
        &self,
        installation_id: Uuid,
        scope: &ModuleInstallationScope,
    ) -> Result<(), ModuleInstallationError> {
        self.store
            .mark_permission_registration_complete(installation_id, scope)
            .await
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
        self.publish_and_commit_reserved(artifact, &staged, evidence, command, request_digest)
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
        self.publish_and_commit_reserved(artifact, &staged, evidence, command, request_digest)
            .await
    }

    async fn publish_and_commit_reserved(
        &self,
        artifact: &InstalledModuleArtifact,
        staged: &StagedArtifactBlob,
        evidence: &ArtifactVerificationEvidence,
        command: &ArtifactAdmissionCommand,
        request_digest: &str,
    ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
        if let Err(error) = self
            .store
            .reserve_blob_publication(staged, Utc::now())
            .await
        {
            self.blobs.discard_best_effort(staged).await;
            return Err(error);
        }
        if let Err(error) = self.blobs.publish(staged).await {
            self.blobs.discard_best_effort(staged).await;
            self.release_publication_reservation_best_effort(staged)
                .await;
            return Err(error);
        }
        match self
            .store
            .commit_admission(artifact, staged, evidence, command, request_digest)
            .await
        {
            Ok(result) => Ok(result),
            Err(error) => {
                self.release_publication_reservation_best_effort(staged)
                    .await;
                Err(error)
            }
        }
    }

    async fn release_publication_reservation_best_effort(&self, staged: &StagedArtifactBlob) {
        // The caller is already returning the publish/commit failure. A
        // failed release is bounded by the reservation lease and must not
        // replace that primary error.
        let _ = self
            .store
            .release_blob_publication_reservation(staged)
            .await;
    }
}

/// Owner-run admission reverification after a trust-root or policy change.
///
/// The service is constructed with the isolated verifier and the policy
/// revisions currently selected by the control plane. A command can identify
/// only a stored installation and expected revision; it cannot submit
/// replacement evidence. Durable replay is resolved before verifier work. A
/// decision evaluated under exactly the current policy revisions persists as
/// `admitted` or `failed`; a stale-policy or signer-less decision is rejected
/// without replacing the stored evidence.
#[derive(Clone)]
pub struct ArtifactAdmissionReverificationService {
    store: SeaOrmArtifactInstallationStore,
    verifier: Arc<dyn TrustVerifier>,
    trust_policy: AdmissionReverificationPolicyRevision,
}

impl ArtifactAdmissionReverificationService {
    pub fn new(
        store: SeaOrmArtifactInstallationStore,
        verifier: Arc<dyn TrustVerifier>,
        trust_policy: AdmissionReverificationPolicyRevision,
    ) -> Self {
        Self {
            store,
            verifier,
            trust_policy,
        }
    }

    /// Lists admissions whose durable evidence predates this service's
    /// trust/capability policy revisions, so a maintenance worker can
    /// drive [`Self::reverify_admission`] for each with its own
    /// authenticated command context. `limit` must be in
    /// `1..=MAX_ADMISSION_REVERIFICATION_CANDIDATES`.
    pub async fn list_stale_candidates(
        &self,
        limit: u32,
    ) -> Result<Vec<ArtifactAdmissionReverificationCandidate>, ModuleInstallationError> {
        self.store
            .list_stale_admission_reverification_candidates(&self.trust_policy, limit)
            .await
    }

    pub async fn reverify_admission(
        &self,
        request: ArtifactAdmissionReverificationCommand,
    ) -> Result<u64, ModuleInstallationError> {
        validate_admission_reverification(&request, &self.trust_policy)?;
        let resulting_revision = request.expected_revision.checked_add(1).ok_or_else(|| {
            ModuleInstallationError::AdmissionRevisionConflict(
                "resulting revision exceeds database integer range".into(),
            )
        })?;
        if i64::try_from(resulting_revision).is_err() {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "resulting revision exceeds database integer range".into(),
            ));
        }
        let request_digest =
            admission_reverification_request_digest(&request, &self.trust_policy)?;
        if let Some(revision) = self
            .store
            .find_admission_reverification_replay(&request, &request_digest, resulting_revision)
            .await?
        {
            return Ok(revision);
        }

        let target = self
            .store
            .load_admission_reverification_target(&request)
            .await?;
        let verification_request = TrustVerificationRequest {
            reference: target.reference.clone(),
            descriptor: target.descriptor.clone(),
            trust_policy_revision: self.trust_policy.trust_policy_revision,
            capability_policy_revision: self.trust_policy.capability_policy_revision,
            expected_alloy_workspace_provenance: None,
        };
        let decision = self
            .verifier
            .verify(verification_request.clone())
            .await
            .map_err(ModuleInstallationError::TrustVerification)?;
        if decision.trust_policy_revision != verification_request.trust_policy_revision
            || decision.capability_policy_revision
                != verification_request.capability_policy_revision
        {
            return Err(ModuleInstallationError::VerificationMismatch(
                "verification decision was not produced under the current policy revisions"
                    .into(),
            ));
        }
        if decision.signer_identity.trim().is_empty() {
            return Err(ModuleInstallationError::VerificationMismatch(
                "verification decision does not identify its signer".into(),
            ));
        }
        let admitted = decision.admitted_for(&verification_request);
        let evidence = ArtifactVerificationEvidence::from_decision_parts(
            &target.reference,
            &target.descriptor,
            &target.media_type,
            decision,
            self.store.infrastructure.now(),
        );
        self.store
            .persist_admission_reverification(
                &request,
                &target,
                &evidence,
                admitted,
                &request_digest,
            )
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
            infrastructure: ControlPlaneInfrastructure::default(),
        }
    }

    pub fn with_infrastructure(mut self, infrastructure: ControlPlaneInfrastructure) -> Self {
        self.infrastructure = infrastructure;
        self
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
        if let Some(replay) = self
            .admission
            .find_admission_replay(&command, &request_digest)
            .await?
        {
            return Ok(self
                .complete_permission_registration(
                    replay.result,
                    &command.scope,
                    &replay.descriptor,
                )
                .await);
        }
        let reference = command.reference.clone();
        let package = self.registry.fetch(&reference, self.limits).await?;
        let temporary_payload_path = match &package.payload {
            ArtifactPayloadSource::TemporaryFile(path) => Some(path.clone()),
            ArtifactPayloadSource::Bytes(_) => None,
        };
        let result = self
            .admit_fetched_package(command, request_digest, package)
            .await;
        if let Some(path) = temporary_payload_path {
            remove_staged_file_best_effort(&path).await;
        }
        result
    }

    async fn admit_fetched_package(
        &self,
        command: ArtifactAdmissionCommand,
        request_digest: String,
        package: ModuleArtifactPackage,
    ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
        if package.reference != command.reference {
            return Err(ModuleInstallationError::RegistryIdentityMismatch {
                requested: command.reference.canonical(),
                received: package.reference.canonical(),
            });
        }
        package.verify(self.limits).await?;
        if package.descriptor.payload_kind == ArtifactPayloadKind::StaticPromoted {
            return Err(ModuleInstallationError::StaticPromotionRequired);
        }
        validate_sandbox_policy_for_admission(&package.descriptor, &command.sandbox_policy)?;
        let verification_request = TrustVerificationRequest {
            reference: package.reference.clone(),
            descriptor: package.descriptor.clone(),
            trust_policy_revision: self.trust_policy.trust_policy_revision,
            capability_policy_revision: self.trust_policy.capability_policy_revision,
            expected_alloy_workspace_provenance: None,
        };
        let decision = self
            .verifier
            .verify(verification_request.clone())
            .await
            .map_err(ModuleInstallationError::TrustVerification)?;
        if !decision.admitted_for(&verification_request) {
            return Err(ModuleInstallationError::VerificationMismatch(
                "verification decision is not admitted for the requested policy revisions".into(),
            ));
        }
        let release = package.release_ref();
        let installed_at = self.infrastructure.now();
        let artifact = InstalledModuleArtifact {
            installation_id: self.infrastructure.new_id(),
            data_owner_id: self.infrastructure.new_id(),
            namespace_instance_id: package
                .descriptor
                .persistence_contract
                .as_ref()
                .map(|_| self.infrastructure.new_id()),
            secret_instance_id: package
                .descriptor
                .capabilities
                .iter()
                .any(|capability| capability.as_str() == "platform.secrets")
                .then(|| self.infrastructure.new_id()),
            settings_instance_id: self.infrastructure.new_id(),
            scope: command.scope.clone(),
            reference: package.reference,
            release,
            descriptor: package.descriptor,
            payload_media_type: package.media_type.clone(),
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
                self.admission
                    .admit_file(
                        &artifact,
                        &package.media_type,
                        &path,
                        &evidence,
                        &command,
                        &request_digest,
                    )
                    .await?
            }
        };
        Ok(self
            .complete_permission_registration(result, &command.scope, &artifact.descriptor)
            .await)
    }

    async fn complete_permission_registration(
        &self,
        mut result: ArtifactAdmissionResult,
        scope: &ModuleInstallationScope,
        descriptor: &ModuleArtifactDescriptor,
    ) -> ArtifactAdmissionResult {
        if result.permission_registration != ArtifactPermissionRegistrationStatus::Pending {
            return result;
        }
        if self
            .register_admitted_permissions(result.installation_id, scope, descriptor)
            .await
            .is_ok()
            && self
                .admission
                .mark_permission_registration_complete(result.installation_id, scope)
                .await
                .is_ok()
        {
            result.permission_registration = ArtifactPermissionRegistrationStatus::Registered;
        }
        result
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
            .admit_release_permissions(ReleasePermissionAdmissionRequest {
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
            })?;

        self.permission_registrar
            .project_scoped_permissions(ScopedPermissionProjectionRequest {
                scope,
                installation_id,
                module_slug: descriptor.slug.clone(),
                release_digest: descriptor.artifact_digest.clone(),
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
    #[error("artifact admission limits must be positive")]
    InvalidAdmissionLimits,
    #[error("artifact payload must not be empty")]
    EmptyPayload,
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
    TrustVerification(#[from] TrustVerificationError),
    #[error("artifact trust verification did not admit the artifact: {0}")]
    VerificationMismatch(String),
    #[error("artifact permission registration failed: {0}")]
    PermissionRegistration(String),
}

fn expected_media_types_for(kind: ArtifactPayloadKind) -> &'static str {
    match kind {
        ArtifactPayloadKind::Rhai => {
            "application/vnd.rustok.rhai.source.v1 or application/vnd.rustok.rhai.workspace.v1"
        }
        _ => kind.oci_layer_media_type(),
    }
}

fn valid_media_type_for(kind: ArtifactPayloadKind, media_type: &str) -> bool {
    kind.supports_media_type(media_type)
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
    validate_lifecycle_command(
        request.installation_id,
        &request.scope,
        request.expected_revision,
        &request.context,
        &request.reason,
    )?;
    if !request.checkpoint.is_object() {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "migration checkpoint requires an object value".into(),
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

/// Stable owner-command identity for checkpoint idempotency. The checkpoint
/// payload stays private to the installation row; the operation receipt holds
/// only this digest and the authenticated command facts.
fn migration_checkpoint_request_digest(
    request: &ArtifactMigrationCheckpointRequest,
) -> Result<String, ModuleInstallationError> {
    let envelope = serde_json::json!({
        "installation_id": request.installation_id,
        "scope": &request.scope,
        "expected_revision": request.expected_revision,
        "checkpoint": &request.checkpoint,
        "has_irreversible_migration": request.has_irreversible_migration,
        "actor_id": request.context.actor_id,
    });
    let bytes = serde_json::to_vec(&envelope)
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(sha256_digest(&bytes))
}

#[derive(Clone, Debug)]
struct StoredAdmissionReverificationTarget {
    reference: OciArtifactReference,
    descriptor: ModuleArtifactDescriptor,
    payload_digest: String,
    media_type: String,
}

fn admission_reverification_status_allowed(status: &str) -> bool {
    matches!(status, "admitted" | "installed" | "inactive" | "failed")
}

async fn ensure_no_active_transition_for_reverification<C: ConnectionTrait>(
    connection: &C,
    scope: &ModuleInstallationScope,
    slug: &str,
) -> Result<(), ModuleInstallationError> {
    let tenant_id = match scope {
        ModuleInstallationScope::Platform => None,
        ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
    };
    let active_checkpoints =
        TransitionCheckpointStore::list_active_checkpoints_for_tenant(connection, tenant_id)
            .await
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    if let Some(active_checkpoint) = active_checkpoints
        .into_iter()
        .find(|checkpoint| checkpoint.module_slug == slug && !checkpoint.state.is_terminal())
    {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(format!(
            "admission reverification locked: active release transition is in progress for '{slug}' (state: {})",
            active_checkpoint.state.name(),
        )));
    }
    Ok(())
}

fn validate_admission_reverification(
    request: &ArtifactAdmissionReverificationCommand,
    trust_policy: &AdmissionReverificationPolicyRevision,
) -> Result<(), ModuleInstallationError> {
    let scope_tenant_id = match &request.scope {
        ModuleInstallationScope::Platform => None,
        ModuleInstallationScope::Tenant { tenant_id } if !tenant_id.is_nil() => Some(*tenant_id),
        ModuleInstallationScope::Tenant { .. } => {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission reverification requires a valid scope and matching command context"
                    .into(),
            ));
        }
    };
    if trust_policy.trust_policy_revision == 0 || trust_policy.capability_policy_revision == 0 {
        return Err(ModuleInstallationError::VerificationMismatch(
            "current trust and capability policy revisions must be positive".into(),
        ));
    }
    if request.installation_id.is_nil()
        || request.expected_revision == 0
        || request.expected_revision >= i64::MAX as u64
        || request.context.tenant_id != scope_tenant_id
        || request.context.validate().is_err()
    {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "admission reverification requires a validated command context, a matching scope, and a positive revision".into(),
        ));
    }
    Ok(())
}

/// Semantic identity for durable reverification replay. Trace and correlation
/// metadata are audit facts from the first committed attempt, not inputs to
/// the owner decision, so changing them on retry cannot manufacture a
/// conflict. The current policy revisions are semantic inputs: asking for a
/// reverification under different revisions is a different command.
fn admission_reverification_request_digest(
    request: &ArtifactAdmissionReverificationCommand,
    trust_policy: &AdmissionReverificationPolicyRevision,
) -> Result<String, ModuleInstallationError> {
    #[derive(Serialize)]
    struct Fingerprint<'a> {
        installation_id: Uuid,
        scope: &'a ModuleInstallationScope,
        expected_revision: u64,
        actor_id: Uuid,
        trust_policy_revision: u64,
        capability_policy_revision: u64,
    }

    let bytes = serde_json::to_vec(&Fingerprint {
        installation_id: request.installation_id,
        scope: &request.scope,
        expected_revision: request.expected_revision,
        actor_id: request.context.actor_id,
        trust_policy_revision: trust_policy.trust_policy_revision,
        capability_policy_revision: trust_policy.capability_policy_revision,
    })
    .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    Ok(sha256_digest(&bytes))
}

fn admission_reverification_scope(
    scope: &ModuleInstallationScope,
) -> Result<(&'static str, String), ModuleInstallationError> {
    match scope {
        ModuleInstallationScope::Platform => Ok(("platform", "platform".to_string())),
        ModuleInstallationScope::Tenant { tenant_id } if !tenant_id.is_nil() => {
            Ok(("tenant", tenant_id.to_string()))
        }
        ModuleInstallationScope::Tenant { .. } => {
            Err(ModuleInstallationError::AdmissionRevisionConflict(
                "admission reverification requires a valid tenant scope".into(),
            ))
        }
    }
}

#[derive(Debug)]
struct AdmissionReverificationReceipt {
    request_digest: String,
    installation_id: Uuid,
    expected_revision: u64,
    resulting_revision: u64,
}

async fn find_admission_reverification_receipt<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    scope_kind: &str,
    scope_tenant_key: &str,
    actor_id: Uuid,
    idempotency_key: Uuid,
) -> Result<Option<AdmissionReverificationReceipt>, ModuleInstallationError> {
    let placeholders = match backend {
        SupportedBackend::Postgres => ("$1", "$2", "$3", "$4"),
        SupportedBackend::Sqlite => ("?1", "?2", "?3", "?4"),
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT request_digest, installation_id, expected_revision, resulting_revision \
                 FROM module_artifact_admission_reverification_operations \
                 WHERE scope_kind = {} AND scope_tenant_key = {} AND actor_id = {} AND idempotency_key = {}",
                placeholders.0, placeholders.1, placeholders.2, placeholders.3,
            ),
            vec![
                scope_kind.into(),
                scope_tenant_key.to_owned().into(),
                backend.uuid_value(actor_id),
                backend.uuid_value(idempotency_key),
            ],
        ))
        .await
        .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
    row.map(|row| {
        let expected_revision: i64 = row
            .try_get("", "expected_revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        let resulting_revision: i64 = row
            .try_get("", "resulting_revision")
            .map_err(|error| ModuleInstallationError::Store(error.to_string()))?;
        Ok(AdmissionReverificationReceipt {
            request_digest: row
                .try_get("", "request_digest")
                .map_err(|error| ModuleInstallationError::Store(error.to_string()))?,
            installation_id: required_uuid_from_row(&row, "installation_id", backend)?,
            expected_revision: u64::try_from(expected_revision).map_err(|_| {
                ModuleInstallationError::Store(
                    "admission reverification receipt expected revision is invalid".into(),
                )
            })?,
            resulting_revision: u64::try_from(resulting_revision).map_err(|_| {
                ModuleInstallationError::Store(
                    "admission reverification receipt resulting revision is invalid".into(),
                )
            })?,
        })
    })
    .transpose()
}

fn replay_admission_reverification(
    receipt: AdmissionReverificationReceipt,
    request: &ArtifactAdmissionReverificationCommand,
    request_digest: &str,
    resulting_revision: u64,
) -> Result<u64, ModuleInstallationError> {
    if receipt.request_digest != request_digest
        || receipt.installation_id != request.installation_id
        || receipt.expected_revision != request.expected_revision
        || receipt.resulting_revision != resulting_revision
    {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "admission reverification idempotency key was already used for a different request"
                .into(),
        ));
    }
    Ok(receipt.resulting_revision)
}

fn admission_reverification_receipt_insert_sql(backend: SupportedBackend) -> String {
    let placeholders = (1..=11)
        .map(|index| backend.placeholder(index))
        .collect::<Vec<_>>();
    format!(
        "INSERT INTO module_artifact_admission_reverification_operations \
         (scope_kind, scope_tenant_key, actor_id, idempotency_key, trace_id, correlation_id, request_digest, installation_id, expected_revision, resulting_revision, committed_at) \
         VALUES ({}) ON CONFLICT DO NOTHING",
        placeholders.join(", "),
    )
}

fn validate_lifecycle_command(
    installation_id: Uuid,
    scope: &ModuleInstallationScope,
    expected_revision: u64,
    context: &ModuleCommandContext,
    reason: &str,
) -> Result<(), ModuleInstallationError> {
    let scope_tenant_id = match scope {
        ModuleInstallationScope::Platform => None,
        ModuleInstallationScope::Tenant { tenant_id } if !tenant_id.is_nil() => Some(*tenant_id),
        ModuleInstallationScope::Tenant { .. } => {
            return Err(ModuleInstallationError::AdmissionRevisionConflict(
                "lifecycle command requires a valid scope and matching command context".into(),
            ));
        }
    };
    if installation_id.is_nil()
        || expected_revision == 0
        || expected_revision > i64::MAX as u64
        || reason.trim().is_empty()
        || context.tenant_id != scope_tenant_id
        || context.validate().is_err()
    {
        return Err(ModuleInstallationError::AdmissionRevisionConflict(
            "lifecycle command requires validated command evidence, a matching scope, a positive revision, and a non-empty reason".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        sync::{Arc, Mutex},
    };

    use chrono::{DateTime, Utc};
    use rustok_core::MigrationSource;
    use rustok_sandbox::{CapabilityGrant, CapabilityName, ExecutionPhase, SandboxPolicy};
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use serde_json::json;

    use super::*;
    use crate::{
        ArtifactModuleKind, ControlPlaneClock, ControlPlaneIdGenerator, TrustEvidenceKind,
        TrustEvidenceReference,
    };

    fn trust_evidence(digest_character: char) -> Vec<TrustEvidenceReference> {
        [
            TrustEvidenceKind::Signature,
            TrustEvidenceKind::Provenance,
            TrustEvidenceKind::Sbom,
        ]
        .into_iter()
        .map(|kind| TrustEvidenceReference {
            kind,
            reference: format!("test://verification/{}-{}", kind.as_str(), digest_character),
            digest: format!("sha256:{}", digest_character.to_string().repeat(64)),
        })
        .collect()
    }

    fn lifecycle_context(tenant_id: Option<Uuid>) -> ModuleCommandContext {
        ModuleCommandContext {
            actor_id: Uuid::new_v4(),
            tenant_id,
            trace_id: "test:artifact-lifecycle".to_string(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        }
    }

    #[test]
    fn positive_revision_rejects_zero_and_negative_stored_values() {
        assert_eq!(
            positive_revision(1, "test").expect("positive revision"),
            1
        );
        assert!(matches!(
            positive_revision(0, "test"),
            Err(ModuleInstallationError::Store(message))
                if message == "test revision is not positive"
        ));
        assert!(matches!(
            positive_revision(-1, "test"),
            Err(ModuleInstallationError::Store(message))
                if message == "test revision is not positive"
        ));
    }

    #[tokio::test]
    async fn admission_limits_must_be_positive_and_payload_must_not_be_empty() {
        assert!(matches!(
            ArtifactAdmissionLimits::new(0, 1),
            Err(ModuleInstallationError::InvalidAdmissionLimits)
        ));
        assert!(matches!(
            ArtifactAdmissionLimits::new(1, 0),
            Err(ModuleInstallationError::InvalidAdmissionLimits)
        ));
        assert!(matches!(
            ArtifactAdmissionLimits {
                max_descriptor_bytes: 0,
                max_payload_bytes: 1,
            }
            .validate(),
            Err(ModuleInstallationError::InvalidAdmissionLimits)
        ));

        let mut empty_package = package(ArtifactPayloadKind::Rhai);
        empty_package.payload = ArtifactPayloadSource::Bytes(Vec::new());
        empty_package.descriptor.artifact_digest = sha256_digest(&[]);
        assert!(matches!(
            empty_package.verify(ArtifactAdmissionLimits::default()).await,
            Err(ModuleInstallationError::EmptyPayload)
        ));
    }

    #[test]
    fn migration_checkpoint_rejects_oversized_owner_metadata() {
        let request = ArtifactMigrationCheckpointRequest {
            installation_id: Uuid::new_v4(),
            scope: ModuleInstallationScope::Platform,
            expected_revision: 1,
            checkpoint: json!({ "payload": "x".repeat(MAX_ARTIFACT_MIGRATION_CHECKPOINT_BYTES) }),
            has_irreversible_migration: false,
            context: lifecycle_context(None),
            reason: "record test checkpoint".to_string(),
        };

        assert!(matches!(
            validate_migration_checkpoint_request(&request),
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
    }

    #[test]
    fn lifecycle_command_requires_non_nil_identities_and_tenant_scope() {
        let installation_id = Uuid::new_v4();
        let tenant_id = Uuid::new_v4();
        let valid = || {
            validate_lifecycle_command(
                installation_id,
                &ModuleInstallationScope::Tenant { tenant_id },
                1,
                &lifecycle_context(Some(tenant_id)),
                "operator request",
            )
        };
        assert!(valid().is_ok());
        assert!(
            validate_lifecycle_command(
                Uuid::nil(),
                &ModuleInstallationScope::Platform,
                1,
                &lifecycle_context(None),
                "operator request",
            )
            .is_err()
        );
        assert!(
            validate_lifecycle_command(
                installation_id,
                &ModuleInstallationScope::Tenant {
                    tenant_id: Uuid::nil(),
                },
                1,
                &lifecycle_context(None),
                "operator request",
            )
            .is_err()
        );
        assert!(
            validate_lifecycle_command(
                installation_id,
                &ModuleInstallationScope::Platform,
                1,
                &ModuleCommandContext {
                    actor_id: Uuid::nil(),
                    ..lifecycle_context(None)
                },
                "operator request",
            )
            .is_err()
        );
        assert!(
            validate_lifecycle_command(
                installation_id,
                &ModuleInstallationScope::Platform,
                1,
                &ModuleCommandContext {
                    idempotency_key: Uuid::nil(),
                    ..lifecycle_context(None)
                },
                "operator request",
            )
            .is_err()
        );
        assert!(
            validate_lifecycle_command(
                installation_id,
                &ModuleInstallationScope::Platform,
                u64::MAX,
                &lifecycle_context(None),
                "operator request",
            )
            .is_err()
        );
    }

    #[test]
    fn admission_command_requires_a_scope_matched_command_context() {
        let reference = OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample_module".to_string(),
            digest: format!("sha256:{}", "a".repeat(64)),
        };
        let tenant_id = Uuid::new_v4();
        let tenant_command = ArtifactAdmissionCommand {
            reference: reference.clone(),
            scope: ModuleInstallationScope::Tenant { tenant_id },
            dependency_lock: empty_dependency_lock(),
            sandbox_policy: SandboxPolicy::default(),
            context: lifecycle_context(None),
        };
        assert!(matches!(
            tenant_command.validate(),
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        let platform_command = ArtifactAdmissionCommand {
            reference,
            scope: ModuleInstallationScope::Platform,
            dependency_lock: empty_dependency_lock(),
            sandbox_policy: SandboxPolicy::default(),
            context: lifecycle_context(Some(tenant_id)),
        };
        assert!(matches!(
            platform_command.validate(),
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
    }

    #[tokio::test]
    async fn rollback_predecessor_selection_uses_one_active_non_uninstalled_serving_release() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE module_artifact_installations (\
                installation_id TEXT PRIMARY KEY, scope_kind TEXT NOT NULL, tenant_id TEXT NULL,\
                slug TEXT NOT NULL\
             )",
            "CREATE TABLE module_artifact_admissions (\
                installation_id TEXT PRIMARY KEY, status TEXT NOT NULL\
             )",
            "CREATE TABLE module_artifact_uninstall_operations (\
                installation_id TEXT PRIMARY KEY\
             )",
        ] {
            database
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("fixture schema");
        }

        let package = package(ArtifactPayloadKind::Rhai);
        let artifact = InstalledModuleArtifact {
            installation_id: Uuid::new_v4(),
            data_owner_id: Uuid::new_v4(),
            namespace_instance_id: package
                .descriptor
                .persistence_contract
                .as_ref()
                .map(|_| Uuid::new_v4()),
            secret_instance_id: package
                .descriptor
                .capabilities
                .iter()
                .any(|capability| capability.as_str() == "platform.secrets")
                .then(Uuid::new_v4),
            settings_instance_id: Uuid::new_v4(),
            scope: ModuleInstallationScope::Platform,
            reference: package.reference.clone(),
            release: package.descriptor.release_ref(),
            descriptor: package.descriptor,
            payload_media_type: package.media_type,
            dependency_lock: empty_dependency_lock(),
            capability_grant_revision: 1,
            installed_at: Utc::now(),
        };
        let active = Uuid::new_v4();
        let inactive = Uuid::new_v4();
        let uninstalled = Uuid::new_v4();
        let conflicting = Uuid::new_v4();
        for (installation_id, status) in [
            (active, "active"),
            (inactive, "inactive"),
            (uninstalled, "active"),
        ] {
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO module_artifact_installations \
                     (installation_id, scope_kind, tenant_id, slug) VALUES (?1, 'platform', NULL, ?2)"
                        .to_string(),
                    vec![
                        installation_id.to_string().into(),
                        artifact.release.slug.clone().into(),
                    ],
                ))
                .await
                .expect("installation fixture");
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO module_artifact_admissions (installation_id, status) VALUES (?1, ?2)"
                        .to_string(),
                    vec![installation_id.to_string().into(), status.into()],
                ))
                .await
                .expect("admission fixture");
        }
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_uninstall_operations (installation_id) VALUES (?1)"
                    .to_string(),
                vec![uninstalled.to_string().into()],
            ))
            .await
            .expect("uninstall fixture");

        assert_eq!(
            active_predecessor_installation(
                &database,
                &artifact.scope,
                &artifact.release.slug,
                SupportedBackend::Sqlite,
            )
            .await
            .expect("one serving predecessor"),
            Some(active)
        );

        for (installation_id, status) in [(conflicting, "active")] {
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO module_artifact_installations \
                     (installation_id, scope_kind, tenant_id, slug) VALUES (?1, 'platform', NULL, ?2)"
                        .to_string(),
                    vec![
                        installation_id.to_string().into(),
                        artifact.release.slug.clone().into(),
                    ],
                ))
                .await
                .expect("conflicting installation fixture");
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "INSERT INTO module_artifact_admissions (installation_id, status) VALUES (?1, ?2)"
                        .to_string(),
                    vec![installation_id.to_string().into(), status.into()],
                ))
                .await
                .expect("conflicting admission fixture");
        }
        assert!(matches!(
            active_predecessor_installation(
                &database,
                &artifact.scope,
                &artifact.release.slug,
                SupportedBackend::Sqlite,
            )
            .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(message))
                if message.contains("multiple active installations")
        ));

        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = 'inactive' WHERE installation_id = ?1"
                    .to_string(),
                vec![conflicting.to_string().into()],
            ))
            .await
            .expect("clear conflicting predecessor");
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = 'inactive' WHERE installation_id = ?1"
                    .to_string(),
                vec![active.to_string().into()],
            ))
            .await
            .expect("deactivate predecessor");
        assert_eq!(
            active_predecessor_installation(
                &database,
                &artifact.scope,
                &artifact.release.slug,
                SupportedBackend::Sqlite,
            )
            .await
            .expect("no serving predecessor"),
            None
        );
    }

    struct FixtureRegistry(ModuleArtifactPackage);

    struct AllowTrustVerifier;

    struct FixedClock(DateTime<Utc>);

    impl ControlPlaneClock for FixedClock {
        fn now(&self) -> DateTime<Utc> {
            self.0.to_owned()
        }
    }

    struct FixedId(Uuid);

    impl ControlPlaneIdGenerator for FixedId {
        fn new_id(&self) -> Uuid {
            self.0
        }
    }

    #[async_trait]
    impl TrustVerifier for AllowTrustVerifier {
        async fn verify(
            &self,
            request: TrustVerificationRequest,
        ) -> Result<TrustVerificationDecision, TrustVerificationError> {
            Ok(TrustVerificationDecision {
                signer_identity: "test.signer.example".to_string(),
                trust_policy_revision: request.trust_policy_revision,
                capability_policy_revision: request.capability_policy_revision,
                signature_verified: true,
                provenance_verified: true,
                sbom_verified: true,
                license_policy_verified: true,
                vulnerability_policy_verified: true,
                evidence: trust_evidence('a'),
            })
        }
    }

    #[derive(Clone, Copy)]
    enum ReverificationVerifierMode {
        Unavailable,
        Reject,
        StalePolicy,
    }

    struct ReverificationTrustVerifier {
        mode: ReverificationVerifierMode,
    }

    #[async_trait]
    impl TrustVerifier for ReverificationTrustVerifier {
        async fn verify(
            &self,
            request: TrustVerificationRequest,
        ) -> Result<TrustVerificationDecision, TrustVerificationError> {
            match self.mode {
                ReverificationVerifierMode::Unavailable => Err(
                    TrustVerificationError::Unavailable(
                        "reverification verifier must not be called".to_string(),
                    ),
                ),
                ReverificationVerifierMode::Reject => Ok(TrustVerificationDecision {
                    signer_identity: "test.signer.example".to_string(),
                    trust_policy_revision: request.trust_policy_revision,
                    capability_policy_revision: request.capability_policy_revision,
                    signature_verified: false,
                    provenance_verified: true,
                    sbom_verified: true,
                    license_policy_verified: true,
                    vulnerability_policy_verified: true,
                    evidence: trust_evidence('b'),
                }),
                ReverificationVerifierMode::StalePolicy => Ok(TrustVerificationDecision {
                    signer_identity: "test.signer.example".to_string(),
                    trust_policy_revision: request.trust_policy_revision.saturating_sub(1),
                    capability_policy_revision: request.capability_policy_revision,
                    signature_verified: true,
                    provenance_verified: true,
                    sbom_verified: true,
                    license_policy_verified: true,
                    vulnerability_policy_verified: true,
                    evidence: trust_evidence('b'),
                }),
            }
        }
    }

    fn unavailable_trust_verifier() -> Arc<dyn TrustVerifier> {
        Arc::new(ReverificationTrustVerifier {
            mode: ReverificationVerifierMode::Unavailable,
        })
    }

    fn rejecting_trust_verifier() -> Arc<dyn TrustVerifier> {
        Arc::new(ReverificationTrustVerifier {
            mode: ReverificationVerifierMode::Reject,
        })
    }

    fn stale_policy_trust_verifier() -> Arc<dyn TrustVerifier> {
        Arc::new(ReverificationTrustVerifier {
            mode: ReverificationVerifierMode::StalePolicy,
        })
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
        async fn admit_release_permissions(
            &self,
            _request: ReleasePermissionAdmissionRequest,
        ) -> Result<(), rustok_api::PortError> {
            Ok(())
        }

        async fn project_scoped_permissions(
            &self,
            _request: ScopedPermissionProjectionRequest,
        ) -> Result<(), rustok_api::PortError> {
            Ok(())
        }

        async fn evaluate_permission_continuity(
            &self,
            request: rustok_api::PermissionContinuityEvaluationRequest,
        ) -> Result<rustok_api::ArtifactPermissionContinuityReceipt, rustok_api::PortError>
        {
            let fingerprint = rustok_api::compute_canonical_authorization_fingerprint(
                &request.candidate_permissions,
            );
            Ok(rustok_api::ArtifactPermissionContinuityReceipt {
                scope: request.scope,
                predecessor_release_digest: request.predecessor_release_digest,
                candidate_release_digest: request.candidate_release_digest,
                authorization_fingerprint: fingerprint,
                rbac_epoch: request.expected_rbac_epoch,
                diff: rustok_api::ArtifactPermissionDiff::default(),
                approved: true,
                receipt_digest: "sha256:mock".to_string(),
            })
        }
    }

    #[derive(Clone, Default)]
    struct RecordingArtifactPermissionRegistrar {
        admissions: Arc<Mutex<Vec<ReleasePermissionAdmissionRequest>>>,
        projections: Arc<Mutex<Vec<ScopedPermissionProjectionRequest>>>,
    }

    #[async_trait]
    impl ArtifactPermissionRegistrationPort for RecordingArtifactPermissionRegistrar {
        async fn admit_release_permissions(
            &self,
            request: ReleasePermissionAdmissionRequest,
        ) -> Result<(), rustok_api::PortError> {
            self.admissions
                .lock()
                .expect("registrar lock")
                .push(request);
            Ok(())
        }

        async fn project_scoped_permissions(
            &self,
            request: ScopedPermissionProjectionRequest,
        ) -> Result<(), rustok_api::PortError> {
            self.projections
                .lock()
                .expect("registrar lock")
                .push(request);
            Ok(())
        }

        async fn evaluate_permission_continuity(
            &self,
            request: rustok_api::PermissionContinuityEvaluationRequest,
        ) -> Result<rustok_api::ArtifactPermissionContinuityReceipt, rustok_api::PortError>
        {
            let fingerprint = rustok_api::compute_canonical_authorization_fingerprint(
                &request.candidate_permissions,
            );
            Ok(rustok_api::ArtifactPermissionContinuityReceipt {
                scope: request.scope,
                predecessor_release_digest: request.predecessor_release_digest,
                candidate_release_digest: request.candidate_release_digest,
                authorization_fingerprint: fingerprint,
                rbac_epoch: request.expected_rbac_epoch,
                diff: rustok_api::ArtifactPermissionDiff::default(),
                approved: true,
                receipt_digest: "sha256:mock".to_string(),
            })
        }
    }

    struct UnavailableArtifactPermissionRegistrar;

    #[async_trait]
    impl ArtifactPermissionRegistrationPort for UnavailableArtifactPermissionRegistrar {
        async fn admit_release_permissions(
            &self,
            _request: ReleasePermissionAdmissionRequest,
        ) -> Result<(), rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable(
                "test.permission_registrar_unavailable",
                "permission registrar is unavailable",
            ))
        }

        async fn project_scoped_permissions(
            &self,
            _request: ScopedPermissionProjectionRequest,
        ) -> Result<(), rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable(
                "test.permission_registrar_unavailable",
                "permission registrar is unavailable",
            ))
        }

        async fn evaluate_permission_continuity(
            &self,
            _request: rustok_api::PermissionContinuityEvaluationRequest,
        ) -> Result<rustok_api::ArtifactPermissionContinuityReceipt, rustok_api::PortError> {
            Err(rustok_api::PortError::unavailable(
                "test.permission_registrar_unavailable",
                "permission registrar is unavailable",
            ))
        }
    }

    struct UnavailableRegistry;

    #[async_trait]
    impl ArtifactRegistry for UnavailableRegistry {
        async fn fetch(
            &self,
            _reference: &OciArtifactReference,
            _limits: ArtifactAdmissionLimits,
        ) -> Result<ModuleArtifactPackage, ModuleInstallationError> {
            Err(ModuleInstallationError::Registry(
                "registry is unavailable".into(),
            ))
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
        async fn find_admission_replay(
            &self,
            _command: &ArtifactAdmissionCommand,
            _request_digest: &str,
        ) -> Result<Option<ArtifactAdmissionReplay>, ModuleInstallationError> {
            Ok(None)
        }

        async fn mark_permission_registration_complete(
            &self,
            _installation_id: Uuid,
            _scope: &ModuleInstallationScope,
        ) -> Result<(), ModuleInstallationError> {
            Ok(())
        }

        async fn reserve_blob_publication(
            &self,
            _staged: &StagedArtifactBlob,
            _reserved_at: DateTime<Utc>,
        ) -> Result<(), ModuleInstallationError> {
            Ok(())
        }

        async fn release_blob_publication_reservation(
            &self,
            _staged: &StagedArtifactBlob,
        ) -> Result<bool, ModuleInstallationError> {
            Ok(false)
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
                permission_registration: initial_permission_registration_status(
                    &artifact.descriptor,
                ),
            })
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

    #[derive(Default)]
    struct RecoveryStoreState {
        referenced: BTreeSet<String>,
        candidates: BTreeMap<String, ArtifactBlobGcCandidate>,
        publication_reservations: BTreeMap<Uuid, (String, DateTime<Utc>)>,
    }

    #[derive(Clone, Default)]
    struct RecoveryStore(Arc<Mutex<RecoveryStoreState>>);

    impl RecoveryStore {
        fn insert_reference_without_fence(&self, digest: &str) {
            self.0
                .lock()
                .expect("store lock")
                .referenced
                .insert(digest.to_string());
        }

        fn remove_reference(&self, digest: &str) {
            self.0.lock().expect("store lock").referenced.remove(digest);
        }
    }

    struct AllowBlobDeletion;

    #[async_trait]
    impl ArtifactBlobRetentionPolicy for AllowBlobDeletion {
        async fn may_delete(&self, _digest: &str) -> Result<bool, ModuleInstallationError> {
            Ok(true)
        }
    }

    #[async_trait]
    impl ArtifactAdmissionStore for RecoveryStore {
        async fn find_admission_replay(
            &self,
            _command: &ArtifactAdmissionCommand,
            _request_digest: &str,
        ) -> Result<Option<ArtifactAdmissionReplay>, ModuleInstallationError> {
            Ok(None)
        }

        async fn mark_permission_registration_complete(
            &self,
            _installation_id: Uuid,
            _scope: &ModuleInstallationScope,
        ) -> Result<(), ModuleInstallationError> {
            Ok(())
        }

        async fn reserve_blob_publication(
            &self,
            staged: &StagedArtifactBlob,
            reserved_at: DateTime<Utc>,
        ) -> Result<(), ModuleInstallationError> {
            let mut state = self.0.lock().expect("store lock");
            if state
                .candidates
                .get(&staged.digest)
                .is_some_and(|candidate| {
                    candidate.state == ArtifactBlobGcCandidateState::Deleting
                })
            {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "blob collection is in progress".into(),
                ));
            }
            state.publication_reservations.insert(
                staged.stage_id,
                (
                    staged.digest.clone(),
                    reserved_at
                        + chrono::Duration::seconds(
                            ARTIFACT_BLOB_PUBLICATION_RESERVATION_LEASE_SECONDS,
                        ),
                ),
            );
            Ok(())
        }

        async fn release_blob_publication_reservation(
            &self,
            staged: &StagedArtifactBlob,
        ) -> Result<bool, ModuleInstallationError> {
            let mut state = self.0.lock().expect("store lock");
            let matches = state
                .publication_reservations
                .get(&staged.stage_id)
                .is_some_and(|(digest, _)| digest == &staged.digest);
            if matches {
                state.publication_reservations.remove(&staged.stage_id);
            }
            Ok(matches)
        }

        async fn commit_admission(
            &self,
            artifact: &InstalledModuleArtifact,
            staged: &StagedArtifactBlob,
            _evidence: &ArtifactVerificationEvidence,
            _command: &ArtifactAdmissionCommand,
            _request_digest: &str,
        ) -> Result<ArtifactAdmissionResult, ModuleInstallationError> {
            let mut state = self.0.lock().expect("store lock");
            let has_reservation = state
                .publication_reservations
                .get(&staged.stage_id)
                .is_some_and(|(digest, _)| digest == &staged.digest);
            if !has_reservation {
                return Err(ModuleInstallationError::AdmissionRevisionConflict(
                    "blob publication reservation is missing".into(),
                ));
            }
            state.publication_reservations.remove(&staged.stage_id);
            match state.candidates.get(&staged.digest).map(|candidate| candidate.state) {
                Some(ArtifactBlobGcCandidateState::Marked) => {
                    state.candidates.remove(&staged.digest);
                }
                Some(ArtifactBlobGcCandidateState::Deleting) => {
                    return Err(ModuleInstallationError::AdmissionRevisionConflict(
                        "blob collection is in progress".into(),
                    ));
                }
                None => {}
            }
            state
                .referenced
                .insert(artifact.descriptor.artifact_digest.clone());
            Ok(ArtifactAdmissionResult {
                installation_id: artifact.installation_id,
                created: true,
                permission_registration: initial_permission_registration_status(
                    &artifact.descriptor,
                ),
            })
        }

        async fn referenced_blob_digests(
            &self,
        ) -> Result<BTreeSet<String>, ModuleInstallationError> {
            Ok(self.0.lock().expect("store lock").referenced.clone())
        }
    }

    #[async_trait]
    impl ArtifactBlobGcCandidateStore for RecoveryStore {
        async fn mark_unreferenced_blob(
            &self,
            digest: &str,
            first_seen_unreferenced_at: DateTime<Utc>,
            grace_ends_at: DateTime<Utc>,
        ) -> Result<ArtifactBlobGcMarkOutcome, ModuleInstallationError> {
            let mut state = self.0.lock().expect("store lock");
            state
                .publication_reservations
                .retain(|_, (reserved_digest, expires_at)| {
                    reserved_digest != digest || *expires_at > first_seen_unreferenced_at
                });
            if state.referenced.contains(digest)
                || state
                    .publication_reservations
                    .values()
                    .any(|(reserved_digest, _)| reserved_digest == digest)
            {
                return Ok(ArtifactBlobGcMarkOutcome::Referenced);
            }
            if let Some(candidate) = state.candidates.get(digest) {
                return Ok(match candidate.state {
                    ArtifactBlobGcCandidateState::Marked => {
                        ArtifactBlobGcMarkOutcome::AlreadyMarked(candidate.clone())
                    }
                    ArtifactBlobGcCandidateState::Deleting => {
                        ArtifactBlobGcMarkOutcome::AlreadyDeleting(candidate.clone())
                    }
                });
            }
            let candidate = ArtifactBlobGcCandidate {
                digest: digest.to_string(),
                first_seen_unreferenced_at,
                grace_ends_at,
                state: ArtifactBlobGcCandidateState::Marked,
                revision: 1,
                delete_started_at: None,
            };
            state
                .candidates
                .insert(digest.to_string(), candidate.clone());
            Ok(ArtifactBlobGcMarkOutcome::Marked(candidate))
        }

        async fn claim_unreferenced_blob_gc(
            &self,
            digest: &str,
            now: DateTime<Utc>,
        ) -> Result<ArtifactBlobGcClaimOutcome, ModuleInstallationError> {
            let mut state = self.0.lock().expect("store lock");
            let Some(candidate) = state.candidates.get(digest).cloned() else {
                return Ok(ArtifactBlobGcClaimOutcome::Missing);
            };
            state
                .publication_reservations
                .retain(|_, (reserved_digest, expires_at)| {
                    reserved_digest != digest || *expires_at > now
                });
            if state
                .publication_reservations
                .values()
                .any(|(reserved_digest, _)| reserved_digest == digest)
            {
                return Ok(ArtifactBlobGcClaimOutcome::Referenced { revoked: false });
            }
            if state.referenced.contains(digest) {
                let revoked = candidate.state == ArtifactBlobGcCandidateState::Marked
                    && state.candidates.remove(digest).is_some();
                return Ok(ArtifactBlobGcClaimOutcome::Referenced { revoked });
            }
            match candidate.state {
                ArtifactBlobGcCandidateState::Marked if now < candidate.grace_ends_at => {
                    return Ok(ArtifactBlobGcClaimOutcome::GraceNotElapsed(candidate));
                }
                ArtifactBlobGcCandidateState::Deleting => {
                    let stale_at = candidate
                        .delete_started_at
                        .expect("deleting fixture candidate start")
                        + chrono::Duration::seconds(ARTIFACT_BLOB_GC_CLAIM_LEASE_SECONDS);
                    if now < stale_at {
                        return Ok(ArtifactBlobGcClaimOutcome::ClaimInProgress(candidate));
                    }
                }
                ArtifactBlobGcCandidateState::Marked => {}
            }
            let claimed = ArtifactBlobGcCandidate {
                state: ArtifactBlobGcCandidateState::Deleting,
                revision: candidate.revision + 1,
                delete_started_at: Some(now),
                ..candidate
            };
            state
                .candidates
                .insert(digest.to_string(), claimed.clone());
            Ok(ArtifactBlobGcClaimOutcome::Claimed(claimed))
        }

        async fn revoke_marked_blob_gc_candidate(
            &self,
            digest: &str,
        ) -> Result<bool, ModuleInstallationError> {
            let mut state = self.0.lock().expect("store lock");
            if !state.referenced.contains(digest) {
                return Ok(false);
            }
            let is_marked = state
                .candidates
                .get(digest)
                .is_some_and(|candidate| candidate.state == ArtifactBlobGcCandidateState::Marked);
            if is_marked {
                state.candidates.remove(digest);
            }
            Ok(is_marked)
        }

        async fn blob_gc_candidates(
            &self,
        ) -> Result<Vec<ArtifactBlobGcCandidate>, ModuleInstallationError> {
            Ok(self
                .0
                .lock()
                .expect("store lock")
                .candidates
                .values()
                .cloned()
                .collect())
        }

        async fn complete_blob_gc(
            &self,
            digest: &str,
            claimed_revision: u64,
        ) -> Result<bool, ModuleInstallationError> {
            let mut state = self.0.lock().expect("store lock");
            if state.referenced.contains(digest)
                || state
                    .publication_reservations
                    .values()
                    .any(|(reserved_digest, _)| reserved_digest == digest)
            {
                return Ok(false);
            }
            let matches_claim = state.candidates.get(digest).is_some_and(|candidate| {
                candidate.state == ArtifactBlobGcCandidateState::Deleting
                    && candidate.revision == claimed_revision
            });
            if matches_claim {
                state.candidates.remove(digest);
            }
            Ok(matches_claim)
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
                localization_catalogs: Vec::new(),
                ui_contributions: Vec::new(),
                persistence_contract: None,
            },
            media_type: kind.oci_layer_media_type().to_string(),
            payload: ArtifactPayloadSource::Bytes(payload),
        }
    }

    fn navigation_package(slug: &str, manifest_marker: char, route: &str) -> ModuleArtifactPackage {
        let mut package = package(ArtifactPayloadKind::Rhai);
        package.reference.repository = format!("modules/{slug}");
        package.reference.digest = format!("sha256:{}", manifest_marker.to_string().repeat(64));
        package.descriptor.slug = slug.to_string();
        let permission = format!("{slug}.ui.view");
        package.descriptor.permissions = vec![crate::ArtifactPermissionDescriptor {
            key: permission.clone(),
            localizations: vec![rustok_api::ArtifactPermissionLocalization {
                locale: "en".to_string(),
                label: "View module UI".to_string(),
                description: "Allows rendering the module UI contribution".to_string(),
            }],
        }];
        let messages = BTreeMap::from([(
            "en".to_string(),
            BTreeMap::from([(
                "navigation.title".to_string(),
                "Module settings".to_string(),
            )]),
        )]);
        let catalog = crate::ArtifactLocalizationCatalog {
            digest: crate::canonical_schema_digest(
                &serde_json::to_value(&messages).expect("localization catalog serializes"),
            ),
            messages,
        };
        package.descriptor.localization_catalogs = vec![catalog.clone()];
        package.descriptor.ui_contributions = vec![crate::ArtifactUiContribution {
            id: "navigation".to_string(),
            surface: rustok_api::ArtifactUiSurface::AdminNavigation,
            localization_digest: catalog.digest,
            permission,
            content: crate::ArtifactUiContributionContent::Navigation {
                title_key: "navigation.title".to_string(),
                route: route.to_string(),
            },
        }];
        package
    }

    #[test]
    fn artifact_ui_resources_cover_navigation_and_storefront_slots() {
        let mut descriptor = navigation_package("sample_module", 'b', "settings").descriptor;
        let catalog = descriptor
            .localization_catalogs
            .first()
            .expect("navigation fixture localization catalog")
            .clone();
        descriptor
            .ui_contributions
            .push(crate::ArtifactUiContribution {
                id: "storefront".to_string(),
                surface: rustok_api::ArtifactUiSurface::StorefrontSlot,
                localization_digest: catalog.digest,
                permission: "sample_module.ui.view".to_string(),
                content: crate::ArtifactUiContributionContent::StorefrontSlot {
                    title_key: "navigation.title".to_string(),
                    slot: "product_details".to_string(),
                },
            });
        descriptor.validate().expect("valid declarative resources");

        assert_eq!(
            artifact_ui_resources(&descriptor),
            BTreeSet::from([
                ArtifactUiResource {
                    kind: ArtifactUiResourceKind::NavigationRoute,
                    key: "settings".to_string(),
                },
                ArtifactUiResource {
                    kind: ArtifactUiResourceKind::StorefrontSlot,
                    key: "product_details".to_string(),
                },
            ])
        );
    }

    fn empty_dependency_lock() -> ModuleDependencyLockGraph {
        ModuleDependencyLockGraph::create(1, Vec::new()).expect("empty dependency lock")
    }

    fn admission_command(
        reference: OciArtifactReference,
        scope: ModuleInstallationScope,
    ) -> ArtifactAdmissionCommand {
        let tenant_id = match &scope {
            ModuleInstallationScope::Platform => None,
            ModuleInstallationScope::Tenant { tenant_id } => Some(*tenant_id),
        };
        ArtifactAdmissionCommand {
            reference,
            scope,
            dependency_lock: empty_dependency_lock(),
            sandbox_policy: SandboxPolicy::default(),
            context: lifecycle_context(tenant_id),
        }
    }

    #[tokio::test]
    async fn reconciler_marks_then_rechecks_before_deleting_published_blobs() {
        let blobs = InMemoryArtifactBlobStore::default();
        let payload = b"orphaned admitted payload";
        let digest = sha256_digest(payload);
        let staged = blobs
            .stage(
                &digest,
                crate::MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE,
                payload,
            )
            .await
            .expect("stage blob");
        blobs.publish(&staged).await.expect("publish blob");
        let store = RecoveryStore::default();
        let reconciler = ArtifactAdmissionReconciler::new(blobs, store.clone());
        let first_seen = DateTime::parse_from_rfc3339("2026-10-25T12:00:00Z")
            .expect("fixed time")
            .with_timezone(&Utc);
        let grace_ends = first_seen
            + chrono::Duration::seconds(ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS);

        let report = reconciler
            .reconcile_published_blobs_at(&AllowBlobDeletion, first_seen)
            .await
            .expect("mark orphaned blob");
        assert_eq!(report.newly_marked, 1);
        assert_eq!(report.deleted, 0);
        assert!(
            reconciler
                .blobs
                .get_verified(&digest)
                .await
                .expect("marked blob remains during grace")
                == payload
        );

        let report = reconciler
            .reconcile_published_blobs_at(
                &AllowBlobDeletion,
                first_seen + chrono::Duration::hours(1),
            )
            .await
            .expect("grace period still blocks collection");
        assert_eq!(report.pending_grace, 1);
        assert_eq!(report.deleted, 0);

        // Simulate an admission that committed after the inventory snapshot
        // but before the final claim. The claim/revoke transaction must see
        // the new live reference and preserve its bytes.
        store.insert_reference_without_fence(&digest);
        let report = reconciler
            .reconcile_published_blobs_at(&AllowBlobDeletion, grace_ends)
            .await
            .expect("final recheck sees new reference");
        assert_eq!(report.referenced, 1);
        assert_eq!(report.revoked, 1);
        assert_eq!(report.deleted, 0);
        assert!(
            reconciler
                .blobs
                .get_verified(&digest)
                .await
                .expect("referenced blob remains")
                == payload
        );

        store.remove_reference(&digest);
        let report = reconciler
            .reconcile_published_blobs_at(&AllowBlobDeletion, grace_ends)
            .await
            .expect("remark unreferenced blob");
        assert_eq!(report.newly_marked, 1);
        assert_eq!(report.deleted, 0);

        let second_grace_ends =
            grace_ends + chrono::Duration::seconds(ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS);
        let report = reconciler
            .reconcile_published_blobs_at(&AllowBlobDeletion, second_grace_ends)
            .await
            .expect("claim and delete after second grace period");
        assert_eq!(report.deleted, 1);
        assert!(matches!(
            reconciler.blobs.get_verified(&digest).await,
            Err(ModuleInstallationError::BlobNotFound(_))
        ));
    }

    #[tokio::test]
    async fn publication_reservation_blocks_blob_gc_until_it_is_released() {
        let blobs = InMemoryArtifactBlobStore::default();
        let payload = b"reserved admitted payload";
        let digest = sha256_digest(payload);
        let staged = blobs
            .stage(
                &digest,
                crate::MODULE_ARTIFACT_RHAI_SOURCE_MEDIA_TYPE,
                payload,
            )
            .await
            .expect("stage blob");
        blobs.publish(&staged).await.expect("publish blob");
        let store = RecoveryStore::default();
        let reconciler = ArtifactAdmissionReconciler::new(blobs, store.clone());
        let first_seen = DateTime::parse_from_rfc3339("2026-10-25T12:00:00Z")
            .expect("fixed time")
            .with_timezone(&Utc);
        let grace_ends = first_seen
            + chrono::Duration::seconds(ARTIFACT_BLOB_GC_GRACE_PERIOD_SECONDS);

        let report = reconciler
            .reconcile_published_blobs_at(&AllowBlobDeletion, first_seen)
            .await
            .expect("mark reserved blob candidate");
        assert_eq!(report.newly_marked, 1);

        store
            .reserve_blob_publication(&staged, grace_ends)
            .await
            .expect("reserve publication");
        let report = reconciler
            .reconcile_published_blobs_at(&AllowBlobDeletion, grace_ends)
            .await
            .expect("live reservation blocks claim");
        assert_eq!(report.referenced, 1);
        assert_eq!(report.deleted, 0);
        assert_eq!(
            reconciler
                .blobs
                .get_verified(&digest)
                .await
                .expect("reserved blob remains"),
            payload
        );

        assert!(
            store
                .release_blob_publication_reservation(&staged)
                .await
                .expect("release publication reservation")
        );
        let report = reconciler
            .reconcile_published_blobs_at(&AllowBlobDeletion, grace_ends)
            .await
            .expect("released reservation allows claim");
        assert_eq!(report.deleted, 1);
    }

    #[tokio::test]
    async fn retention_snapshot_requires_an_explicit_eligible_rule_for_deletion() {
        let now = Utc::now();
        let digest = sha256_digest(b"retained artifact payload");
        let policy = SnapshotArtifactBlobRetentionPolicy::new(now, HashMap::new());

        assert!(
            !policy
                .may_delete(&digest)
                .await
                .expect("missing retention rule fails closed")
        );

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

        assert!(
            policy
                .may_delete(&digest)
                .await
                .expect("expired unprotected rule allows deletion")
        );
    }

    #[tokio::test]
    async fn digest_pinned_package_installs_without_changing_server_source() {
        let package = package(ArtifactPayloadKind::Rhai);
        let reference = package.reference.clone();
        let store = CapturingStore::default();
        let installed_at = DateTime::parse_from_rfc3339("2026-07-20T12:00:00Z")
            .expect("fixed time")
            .with_timezone(&Utc);
        let installation_id =
            Uuid::parse_str("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").expect("fixed UUID");
        let installer = ModuleInstaller::new(
            FixtureRegistry(package),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .with_infrastructure(ControlPlaneInfrastructure::new(
            Arc::new(FixedClock(installed_at.to_owned())),
            Arc::new(FixedId(installation_id)),
        ));

        let admission = installer
            .admit(admission_command(
                reference,
                ModuleInstallationScope::Platform,
            ))
            .await
            .expect("admission");
        assert!(admission.created);
        let installed = store.0.lock().expect("store lock")[0].clone();

        assert_eq!(admission.installation_id, installation_id);
        assert_eq!(installed.installed_at, installed_at);
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
        let admissions = registrar.admissions.lock().expect("registrar lock");
        assert_eq!(admissions.len(), 1);
        assert_eq!(admissions[0].module_slug, "sample_module");
        assert_eq!(admissions[0].permissions.len(), 1);
        assert_eq!(
            admissions[0].permissions[0].key,
            "sample_module.events.handle"
        );
        let projections = registrar.projections.lock().expect("registrar lock");
        assert_eq!(projections.len(), 1);
        assert_eq!(projections[0].installation_id, result.installation_id);
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
    async fn admission_removes_temporary_payload_on_early_rejection() {
        let mut package = package(ArtifactPayloadKind::StaticPromoted);
        let ArtifactPayloadSource::Bytes(payload) = package.payload.clone() else {
            panic!("fixture uses an in-memory payload");
        };
        let path = std::env::temp_dir().join(format!(
            "rustok-admission-payload-{}.tmp",
            Uuid::new_v4()
        ));
        std::fs::write(&path, payload).expect("temporary payload fixture");
        package.payload = ArtifactPayloadSource::TemporaryFile(path.clone());
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
        assert!(!path.exists(), "temporary payload must be removed on every exit");
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
    async fn admission_replay_heals_pending_permission_registration_without_registry() {
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
        let tenant_id = Uuid::new_v4();
        let command = admission_command(
            reference,
            ModuleInstallationScope::Tenant { tenant_id },
        );
        let store = SeaOrmArtifactInstallationStore::new(database);

        let first = ModuleInstaller::new(
            FixtureRegistry(package),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            UnavailableArtifactPermissionRegistrar,
        )
        .admit(command.clone())
        .await
        .expect("admission remains acknowledged when permission projection is pending");
        assert!(first.created);
        assert_eq!(
            first.permission_registration,
            ArtifactPermissionRegistrationStatus::Pending
        );

        let registrar = RecordingArtifactPermissionRegistrar::default();
        let replayed = ModuleInstaller::new(
            UnavailableRegistry,
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            registrar.clone(),
        )
        .admit(command.clone())
        .await
        .expect("registry-free replay completes pending permission registration");
        assert!(!replayed.created);
        assert_eq!(replayed.installation_id, first.installation_id);
        assert_eq!(
            replayed.permission_registration,
            ArtifactPermissionRegistrationStatus::Registered
        );
        assert_eq!(registrar.admissions.lock().expect("registrar lock").len(), 1);
        assert_eq!(
            registrar.projections.lock().expect("registrar lock").len(),
            1
        );

        let replayed_again = ModuleInstaller::new(
            UnavailableRegistry,
            store,
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            UnavailableArtifactPermissionRegistrar,
        )
        .admit(command)
        .await
        .expect("completed permission registration does not need the registrar again");
        assert_eq!(
            replayed_again.permission_registration,
            ArtifactPermissionRegistrationStatus::Registered
        );
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
            context: lifecycle_context(Some(tenant_id)),
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
            context: command.context.clone(),
        };
        let admission = installer.admit(command.clone()).await.expect("admission");
        assert!(admission.created);
        let duplicate = installer
            .admit(command.clone())
            .await
            .expect("idempotent admission");
        assert!(!duplicate.created);
        assert_eq!(duplicate.installation_id, admission.installation_id);
        assert!(matches!(
            installer.admit(conflicting_command).await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        let changed_observability_command = ArtifactAdmissionCommand {
            context: ModuleCommandContext {
                trace_id: "test:changed-admission-observability-context".to_string(),
                correlation_id: Uuid::new_v4(),
                ..command.context.clone()
            },
            ..command.clone()
        };
        let changed_observability_replay = installer
            .admit(changed_observability_command)
            .await
            .expect("changed observability context replays the semantic admission");
        assert!(!changed_observability_replay.created);
        assert_eq!(
            changed_observability_replay.installation_id,
            admission.installation_id
        );
        let installed = InstalledModuleArtifact {
            installation_id: admission.installation_id,
            data_owner_id: Uuid::new_v4(),
            namespace_instance_id: expected_descriptor
                .persistence_contract
                .as_ref()
                .map(|_| Uuid::new_v4()),
            secret_instance_id: expected_descriptor
                .capabilities
                .iter()
                .any(|capability| capability.as_str() == "platform.secrets")
                .then(Uuid::new_v4),
            settings_instance_id: Uuid::new_v4(),
            scope: ModuleInstallationScope::Tenant { tenant_id },
            reference: expected_reference,
            release: expected_descriptor.release_ref(),
            descriptor: expected_descriptor,
            payload_media_type: "application/vnd.rustok.rhai.source.v1".to_string(),
            dependency_lock: expected_lock,
            capability_grant_revision: 1,
            installed_at: Utc::now(),
        };

        let row = database
            .query_one_raw(Statement::from_string(
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
            .query_one_raw(Statement::from_string(
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
            .query_one_raw(Statement::from_string(
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
        let admission_command_receipt = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT actor_id, trace_id, correlation_id, idempotency_key \
                 FROM module_artifact_admission_commands"
                    .to_string(),
            ))
            .await
            .expect("admission command receipt query")
            .expect("admission command receipt");
        assert_eq!(
            String::try_get(&admission_command_receipt, "", "actor_id").expect("receipt actor"),
            command.context.actor_id.to_string()
        );
        assert_eq!(
            String::try_get(&admission_command_receipt, "", "trace_id").expect("receipt trace"),
            command.context.trace_id
        );
        assert_eq!(
            String::try_get(&admission_command_receipt, "", "correlation_id")
                .expect("receipt correlation"),
            command.context.correlation_id.to_string()
        );
        assert_eq!(
            String::try_get(&admission_command_receipt, "", "idempotency_key")
                .expect("receipt idempotency"),
            command.context.idempotency_key.to_string()
        );
        let outbox_count = database
            .query_one_raw(Statement::from_string(
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
        let admitted_event = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM sys_events WHERE event_type = 'module.artifact.admitted'"
                    .to_string(),
            ))
            .await
            .expect("admitted event query")
            .expect("admitted event");
        let admitted_payload: Value = admitted_event
            .try_get("", "payload")
            .expect("admitted event payload");
        let admitted_envelope: rustok_events::EventEnvelope =
            serde_json::from_value(admitted_payload).expect("admitted event envelope");
        assert_eq!(admitted_envelope.actor_id, Some(command.context.actor_id));
        assert_eq!(
            admitted_envelope.correlation_id,
            command.context.correlation_id
        );
        assert_eq!(
            admitted_envelope.trace_id.as_deref(),
            Some(command.context.trace_id.as_str())
        );

        let reverification_policy = AdmissionReverificationPolicyRevision {
            trust_policy_revision: 2,
            capability_policy_revision: 2,
        };
        let reverification_service = ArtifactAdmissionReverificationService::new(
            store.clone(),
            trust_verifier(),
            reverification_policy,
        );
        let replay_reverification_service = ArtifactAdmissionReverificationService::new(
            store.clone(),
            unavailable_trust_verifier(),
            reverification_policy,
        );
        let reverification = ArtifactAdmissionReverificationCommand {
            installation_id: installed.installation_id,
            scope: ModuleInstallationScope::Tenant { tenant_id },
            expected_revision: 1,
            context: lifecycle_context(Some(tenant_id)),
        };
        assert_eq!(
            reverification_service
                .reverify_admission(reverification.clone())
                .await
                .expect("reverification"),
            2
        );
        assert_eq!(
            replay_reverification_service
                .reverify_admission(reverification.clone())
                .await
                .expect("exact reverification replay does not call the verifier"),
            2
        );
        let mut changed_trace_reverification = reverification.clone();
        changed_trace_reverification.context.trace_id = "changed-reverification-trace".to_string();
        changed_trace_reverification.context.correlation_id = Uuid::new_v4();
        assert_eq!(
            replay_reverification_service
                .reverify_admission(changed_trace_reverification)
                .await
                .expect("trace and correlation metadata are not semantic replay inputs"),
            2
        );
        let reverification_outbox_count = database
            .query_one_raw(Statement::from_string(
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
        let reverification_receipt = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT actor_id, trace_id, correlation_id, idempotency_key, expected_revision, resulting_revision \
                 FROM module_artifact_admission_reverification_operations"
                    .to_string(),
            ))
            .await
            .expect("reverification receipt query")
            .expect("reverification receipt");
        assert_eq!(
            String::try_get(&reverification_receipt, "", "actor_id").expect("receipt actor"),
            reverification.context.actor_id.to_string()
        );
        assert_eq!(
            String::try_get(&reverification_receipt, "", "trace_id").expect("receipt trace"),
            reverification.context.trace_id
        );
        assert_eq!(
            String::try_get(&reverification_receipt, "", "correlation_id")
                .expect("receipt correlation"),
            reverification.context.correlation_id.to_string()
        );
        assert_eq!(
            String::try_get(&reverification_receipt, "", "idempotency_key")
                .expect("receipt idempotency"),
            reverification.context.idempotency_key.to_string()
        );
        assert_eq!(
            i64::try_get(&reverification_receipt, "", "expected_revision")
                .expect("receipt expected revision"),
            1
        );
        assert_eq!(
            i64::try_get(&reverification_receipt, "", "resulting_revision")
                .expect("receipt resulting revision"),
            2
        );
        let reverification_event = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM sys_events WHERE event_type = 'module.artifact.reverified'"
                    .to_string(),
            ))
            .await
            .expect("reverification event query")
            .expect("reverification event");
        let reverification_payload: Value = reverification_event
            .try_get("", "payload")
            .expect("reverification event payload");
        let reverification_envelope: rustok_events::EventEnvelope =
            serde_json::from_value(reverification_payload).expect("reverification event envelope");
        assert_eq!(
            reverification_envelope.actor_id,
            Some(reverification.context.actor_id)
        );
        assert_eq!(
            reverification_envelope.correlation_id,
            reverification.context.correlation_id
        );
        assert_eq!(
            reverification_envelope.trace_id.as_deref(),
            Some(reverification.context.trace_id.as_str())
        );
        let checkpoint_request = ArtifactMigrationCheckpointRequest {
            installation_id: installed.installation_id,
            scope: ModuleInstallationScope::Tenant { tenant_id },
            expected_revision: 2,
            checkpoint: json!({ "data_contract_revision": 2 }),
            has_irreversible_migration: true,
            context: lifecycle_context(Some(tenant_id)),
            reason: "record data contract migration".to_string(),
        };
        let checkpoint_actor_id = checkpoint_request.context.actor_id;
        assert_eq!(
            store
                .record_migration_checkpoint(checkpoint_request.clone())
                .await
                .expect("migration checkpoint"),
            3
        );
        assert_eq!(
            store
                .record_migration_checkpoint(checkpoint_request.clone())
                .await
                .expect("idempotent migration checkpoint"),
            3
        );
        assert!(matches!(
            store
                .record_migration_checkpoint(ArtifactMigrationCheckpointRequest {
                    context: lifecycle_context(Some(tenant_id)),
                    ..checkpoint_request
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        let checkpoint_outbox_count = database
            .query_one_raw(Statement::from_string(
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
        let checkpoint_operation = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT revision, request_digest, actor_id \
                 FROM module_artifact_migration_checkpoint_operations"
                    .to_string(),
            ))
            .await
            .expect("checkpoint operation query")
            .expect("checkpoint operation");
        assert_eq!(
            i64::try_get(&checkpoint_operation, "", "revision").expect("checkpoint revision"),
            3
        );
        assert!(
            String::try_get(&checkpoint_operation, "", "request_digest")
                .expect("checkpoint request digest")
                .starts_with("sha256:")
        );
        assert_eq!(
            String::try_get(&checkpoint_operation, "", "actor_id").expect("checkpoint actor"),
            checkpoint_actor_id.to_string()
        );
        let checkpoint_row = database
            .query_one_raw(Statement::from_string(
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
            .execute_raw(Statement::from_sql_and_values(
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
            context: lifecycle_context(Some(tenant_id)),
            reason: "retire runtime bindings".to_string(),
        };
        let deactivation_context = deactivation_request.context.clone();
        let deactivated = store
            .deactivate_artifact(deactivation_request.clone())
            .await
            .expect("deactivate artifact");
        assert_eq!(deactivated.revision, 4);
        let changed_observability_deactivation = ArtifactDeactivationRequest {
            context: ModuleCommandContext {
                trace_id: "test:changed-artifact-lifecycle-observability".to_string(),
                correlation_id: Uuid::new_v4(),
                ..deactivation_request.context.clone()
            },
            ..deactivation_request.clone()
        };
        assert_eq!(
            store
                .deactivate_artifact(changed_observability_deactivation)
                .await
                .expect("changed observability context replays deactivation"),
            deactivated
        );
        assert_eq!(
            store
                .deactivate_artifact(deactivation_request)
                .await
                .expect("idempotent deactivation"),
            deactivated
        );
        let deactivation_outbox_count = database
            .query_one_raw(Statement::from_string(
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
        let deactivation_event = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT payload FROM sys_events \
                 WHERE event_type = 'module.artifact.deactivated'"
                    .to_string(),
            ))
            .await
            .expect("deactivation event query")
            .expect("deactivation event");
        let deactivation_payload: Value = deactivation_event
            .try_get("", "payload")
            .expect("deactivation event payload");
        let deactivation_envelope: rustok_events::EventEnvelope =
            serde_json::from_value(deactivation_payload).expect("deactivation event envelope");
        assert_eq!(
            deactivation_envelope.actor_id,
            Some(deactivation_context.actor_id)
        );
        assert_eq!(deactivation_envelope.tenant_id, tenant_id);
        assert_eq!(
            deactivation_envelope.correlation_id,
            deactivation_context.correlation_id
        );
        assert_eq!(
            deactivation_envelope.trace_id.as_deref(),
            Some(deactivation_context.trace_id.as_str())
        );
        let deactivation_row = database
            .query_one_raw(Statement::from_string(
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
            context: lifecycle_context(Some(tenant_id)),
            reason: "disable tenant intent".to_string(),
        };
        let inherited_tenant_intent = store
            .artifact_tenant_lifecycle_snapshot(installed.installation_id, tenant_id)
            .await
            .expect("load inherited tenant lifecycle state");
        assert!(inherited_tenant_intent.enabled);
        assert_eq!(inherited_tenant_intent.revision, 0);
        assert_eq!(inherited_tenant_intent.expected_revision, 1);
        assert!(matches!(
            store
                .artifact_tenant_lifecycle_snapshot(installed.installation_id, Uuid::new_v4())
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        assert_eq!(
            store
                .disable_artifact_for_tenant(tenant_disable.clone())
                .await
                .expect("disable tenant artifact")
                .revision,
            1
        );
        let changed_reason_tenant_disable = ArtifactTenantDisableRequest {
            reason: "a different disable reason".to_string(),
            ..tenant_disable.clone()
        };
        assert_eq!(
            store
                .disable_artifact_for_tenant(changed_reason_tenant_disable)
                .await
                .expect("free-text reason is audit evidence, not replay identity")
                .revision,
            1
        );
        assert_eq!(
            store
                .disable_artifact_for_tenant(tenant_disable.clone())
                .await
                .expect("idempotent tenant disable")
                .revision,
            1
        );
        let disabled_tenant_intent = store
            .artifact_tenant_lifecycle_snapshot(installed.installation_id, tenant_id)
            .await
            .expect("load disabled tenant lifecycle state");
        assert!(!disabled_tenant_intent.enabled);
        assert_eq!(disabled_tenant_intent.revision, 1);
        assert_eq!(disabled_tenant_intent.expected_revision, 1);
        assert!(matches!(
            store
                .enable_artifact_for_tenant(ArtifactTenantEnableRequest {
                    installation_id: installed.installation_id,
                    tenant_id,
                    expected_revision: 1,
                    context: tenant_disable.context.clone(),
                    reason: tenant_disable.reason.clone(),
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        let tenant_enable = ArtifactTenantEnableRequest {
            installation_id: installed.installation_id,
            tenant_id,
            expected_revision: 1,
            context: lifecycle_context(Some(tenant_id)),
            reason: "restore tenant intent".to_string(),
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
        assert_eq!(
            store
                .disable_artifact_for_tenant(tenant_disable.clone())
                .await
                .expect("replay tenant disable after a later enable")
                .revision,
            1
        );
        let enabled_tenant_intent = store
            .artifact_tenant_lifecycle_snapshot(installed.installation_id, tenant_id)
            .await
            .expect("load enabled tenant lifecycle state");
        assert!(enabled_tenant_intent.enabled);
        assert_eq!(enabled_tenant_intent.revision, 2);
        assert_eq!(enabled_tenant_intent.expected_revision, 2);
        let tenant_disable_outbox = database
            .query_one_raw(Statement::from_string(
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
            .query_one_raw(Statement::from_string(
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
        let tenant_lifecycle_operations = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM module_artifact_tenant_lifecycle_operations"
                    .to_string(),
            ))
            .await
            .expect("tenant lifecycle operation query")
            .expect("tenant lifecycle operation row");
        assert_eq!(
            i64::try_get(&tenant_lifecycle_operations, "", "count")
                .expect("tenant lifecycle operation count"),
            2
        );
        let uninstall_request = ArtifactUninstallRequest {
            installation_id: installed.installation_id,
            scope: ModuleInstallationScope::Tenant { tenant_id },
            expected_revision: 4,
            context: lifecycle_context(Some(tenant_id)),
            reason: "remove inactive selection".to_string(),
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
        assert!(matches!(
            store
                .uninstall_artifact(ArtifactUninstallRequest {
                    installation_id: installed.installation_id,
                    scope: ModuleInstallationScope::Tenant { tenant_id },
                    expected_revision: 5,
                    context: lifecycle_context(Some(tenant_id)),
                    reason: "must not repeat terminal uninstall".to_string(),
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        assert!(matches!(
            store
                .disable_artifact_for_tenant(ArtifactTenantDisableRequest {
                    installation_id: installed.installation_id,
                    tenant_id,
                    expected_revision: 2,
                    context: lifecycle_context(Some(tenant_id)),
                    reason: "must not revive uninstalled tenant intent".to_string(),
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
        assert_eq!(
            store
                .disable_artifact_for_tenant(tenant_disable)
                .await
                .expect("replay tenant disable after uninstall")
                .revision,
            1
        );
        let uninstall_outbox_count = database
            .query_one_raw(Statement::from_string(
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
            replay_reverification_service
                .reverify_admission(ArtifactAdmissionReverificationCommand {
                    installation_id: installed.installation_id,
                    scope: ModuleInstallationScope::Tenant { tenant_id },
                    expected_revision: 5,
                    context: lifecycle_context(Some(tenant_id)),
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));
    }

    #[tokio::test]
    async fn owner_reverification_guards_active_and_stale_policy_before_marking_failed() {
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

        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let tenant_id = Uuid::new_v4();
        let package = package(ArtifactPayloadKind::Rhai);
        let reference = package.reference.clone();
        let expected_manifest_digest = reference.digest.clone();
        let expected_payload_digest = package.descriptor.artifact_digest.clone();
        let admission = ModuleInstaller::new(
            FixtureRegistry(package),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            reference,
            ModuleInstallationScope::Tenant { tenant_id },
        ))
        .await
        .expect("admission");
        let current_policy = AdmissionReverificationPolicyRevision {
            trust_policy_revision: 2,
            capability_policy_revision: 2,
        };
        let reverification_command = |expected_revision| ArtifactAdmissionReverificationCommand {
            installation_id: admission.installation_id,
            scope: ModuleInstallationScope::Tenant { tenant_id },
            expected_revision,
            context: lifecycle_context(Some(tenant_id)),
        };

        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = 'active' WHERE installation_id = ?1"
                    .to_string(),
                vec![admission.installation_id.to_string().into()],
            ))
            .await
            .expect("mark admission active");
        let unavailable_service = ArtifactAdmissionReverificationService::new(
            store.clone(),
            unavailable_trust_verifier(),
            current_policy,
        );
        assert!(matches!(
            unavailable_service
                .reverify_admission(reverification_command(1))
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));

        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_admissions SET status = 'inactive' WHERE installation_id = ?1"
                    .to_string(),
                vec![admission.installation_id.to_string().into()],
            ))
            .await
            .expect("mark admission inactive");
        let stale_policy_service = ArtifactAdmissionReverificationService::new(
            store.clone(),
            stale_policy_trust_verifier(),
            current_policy,
        );
        assert!(matches!(
            stale_policy_service
                .reverify_admission(reverification_command(1))
                .await,
            Err(ModuleInstallationError::VerificationMismatch(_))
        ));
        let unchanged = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT status, revision FROM module_artifact_admissions WHERE installation_id = ?1"
                    .to_string(),
                vec![admission.installation_id.to_string().into()],
            ))
            .await
            .expect("unchanged admission query")
            .expect("unchanged admission row");
        assert_eq!(
            String::try_get(&unchanged, "", "status").expect("unchanged status"),
            "inactive"
        );
        assert_eq!(
            i64::try_get(&unchanged, "", "revision").expect("unchanged revision"),
            1
        );

        let rejecting_service = ArtifactAdmissionReverificationService::new(
            store,
            rejecting_trust_verifier(),
            current_policy,
        );
        assert_eq!(
            rejecting_service
                .reverify_admission(reverification_command(1))
                .await
                .expect("current-policy rejected reverification"),
            2
        );
        let failed = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT status, revision, CAST(verification_evidence AS TEXT) AS verification_evidence \
                 FROM module_artifact_admissions WHERE installation_id = ?1"
                    .to_string(),
                vec![admission.installation_id.to_string().into()],
            ))
            .await
            .expect("failed admission query")
            .expect("failed admission row");
        assert_eq!(
            String::try_get(&failed, "", "status").expect("failed status"),
            "failed"
        );
        assert_eq!(
            i64::try_get(&failed, "", "revision").expect("failed revision"),
            2
        );
        let evidence: ArtifactVerificationEvidence = serde_json::from_str(
            &String::try_get(&failed, "", "verification_evidence").expect("stored evidence"),
        )
        .expect("stored evidence JSON");
        assert_eq!(evidence.manifest_digest, expected_manifest_digest);
        assert_eq!(evidence.payload_digest, expected_payload_digest);
        assert_eq!(evidence.trust_policy_revision, 2);
        assert_eq!(evidence.capability_policy_revision, 2);
        assert!(!evidence.signature_verified);
    }

    #[tokio::test]
    async fn tenant_activation_rejects_a_platform_navigation_collision() {
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

        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let platform_package = navigation_package("platform_menu", 'b', "settings");
        let platform = ModuleInstaller::new(
            FixtureRegistry(platform_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            platform_package.reference.clone(),
            ModuleInstallationScope::Platform,
        ))
        .await
        .expect("platform admission");
        store
            .activate_artifact(ArtifactActivationRequest {
                installation_id: platform.installation_id,
                scope: ModuleInstallationScope::Platform,
                expected_revision: 1,
                context: lifecycle_context(None),
                reason: "activate platform navigation".to_string(),
            })
            .await
            .expect("platform activation");

        let tenant_id = Uuid::new_v4();
        let tenant_package = navigation_package("tenant_menu", 'c', "settings");
        let tenant = ModuleInstaller::new(
            FixtureRegistry(tenant_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            tenant_package.reference.clone(),
            ModuleInstallationScope::Tenant { tenant_id },
        ))
        .await
        .expect("tenant admission");

        assert!(matches!(
            store
                .activate_artifact(ArtifactActivationRequest {
                    installation_id: tenant.installation_id,
                    scope: ModuleInstallationScope::Tenant { tenant_id },
                    expected_revision: 1,
                    context: lifecycle_context(Some(tenant_id)),
                    reason: "activate colliding tenant navigation".to_string(),
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(reason))
                if reason == "activation conflicts with an active artifact UI contribution"
        ));
        let tenant_admission = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT status, revision FROM module_artifact_admissions \
                 WHERE installation_id = ?1"
                    .to_string(),
                vec![tenant.installation_id.to_string().into()],
            ))
            .await
            .expect("tenant admission query")
            .expect("tenant admission row");
        assert_eq!(
            String::try_get(&tenant_admission, "", "status").expect("tenant status"),
            "admitted"
        );
        assert_eq!(
            i64::try_get(&tenant_admission, "", "revision").expect("tenant revision"),
            1
        );
    }

    #[tokio::test]
    async fn platform_activation_rejects_a_tenant_navigation_collision() {
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

        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let tenant_id = Uuid::new_v4();
        let tenant_package = navigation_package("tenant_menu", 'b', "settings");
        let tenant = ModuleInstaller::new(
            FixtureRegistry(tenant_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            tenant_package.reference.clone(),
            ModuleInstallationScope::Tenant { tenant_id },
        ))
        .await
        .expect("tenant admission");
        store
            .activate_artifact(ArtifactActivationRequest {
                installation_id: tenant.installation_id,
                scope: ModuleInstallationScope::Tenant { tenant_id },
                expected_revision: 1,
                context: lifecycle_context(Some(tenant_id)),
                reason: "activate tenant navigation".to_string(),
            })
            .await
            .expect("tenant activation");

        let platform_package = navigation_package("platform_menu", 'c', "settings");
        let platform = ModuleInstaller::new(
            FixtureRegistry(platform_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            platform_package.reference.clone(),
            ModuleInstallationScope::Platform,
        ))
        .await
        .expect("platform admission");

        assert!(matches!(
            store
                .activate_artifact(ArtifactActivationRequest {
                    installation_id: platform.installation_id,
                    scope: ModuleInstallationScope::Platform,
                    expected_revision: 1,
                    context: lifecycle_context(None),
                    reason: "activate colliding platform navigation".to_string(),
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(reason))
                if reason == "activation conflicts with an active artifact UI contribution"
        ));
        let platform_admission = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT status, revision FROM module_artifact_admissions \
                 WHERE installation_id = ?1"
                    .to_string(),
                vec![platform.installation_id.to_string().into()],
            ))
            .await
            .expect("platform admission query")
            .expect("platform admission row");
        assert_eq!(
            String::try_get(&platform_admission, "", "status").expect("platform status"),
            "admitted"
        );
        assert_eq!(
            i64::try_get(&platform_admission, "", "revision").expect("platform revision"),
            1
        );
    }

    #[tokio::test]
    async fn rollback_rejects_a_tenant_collision_with_the_predecessor_ui() {
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

        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let predecessor_package = navigation_package("platform_menu", 'b', "settings");
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
        store
            .activate_artifact(ArtifactActivationRequest {
                installation_id: predecessor.installation_id,
                scope: ModuleInstallationScope::Platform,
                expected_revision: 1,
                context: lifecycle_context(None),
                reason: "activate predecessor navigation".to_string(),
            })
            .await
            .expect("predecessor activation");

        let mut successor_package = predecessor_package.clone();
        successor_package.reference.digest = format!("sha256:{}", "c".repeat(64));
        successor_package.descriptor.version = "2.0.0".to_string();
        let crate::ArtifactUiContributionContent::Navigation { route, .. } =
            &mut successor_package.descriptor.ui_contributions[0].content
        else {
            panic!("fixture must declare navigation");
        };
        *route = "other_settings".to_string();
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
        store
            .activate_artifact(ArtifactActivationRequest {
                installation_id: successor.installation_id,
                scope: ModuleInstallationScope::Platform,
                expected_revision: 1,
                context: lifecycle_context(None),
                reason: "activate successor navigation".to_string(),
            })
            .await
            .expect("successor activation");

        let tenant_id = Uuid::new_v4();
        let tenant_package = navigation_package("tenant_menu", 'd', "settings");
        let tenant = ModuleInstaller::new(
            FixtureRegistry(tenant_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            tenant_package.reference.clone(),
            ModuleInstallationScope::Tenant { tenant_id },
        ))
        .await
        .expect("tenant admission");
        store
            .activate_artifact(ArtifactActivationRequest {
                installation_id: tenant.installation_id,
                scope: ModuleInstallationScope::Tenant { tenant_id },
                expected_revision: 1,
                context: lifecycle_context(Some(tenant_id)),
                reason: "activate tenant navigation".to_string(),
            })
            .await
            .expect("tenant activation");

        assert!(matches!(
            store
                .rollback_artifact(ArtifactRollbackRequest {
                    installation_id: successor.installation_id,
                    scope: ModuleInstallationScope::Platform,
                    expected_revision: 2,
                    context: lifecycle_context(None),
                    reason: "restore colliding predecessor navigation".to_string(),
                    target_capability_grant_revision: 1,
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(reason))
                if reason == "activation conflicts with an active artifact UI contribution"
        ));
        let source_admission = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT status, revision FROM module_artifact_admissions \
                 WHERE installation_id = ?1"
                    .to_string(),
                vec![successor.installation_id.to_string().into()],
            ))
            .await
            .expect("source admission query")
            .expect("source admission row");
        assert_eq!(
            String::try_get(&source_admission, "", "status").expect("source status"),
            "active"
        );
        assert_eq!(
            i64::try_get(&source_admission, "", "revision").expect("source revision"),
            2
        );
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
        let predecessor_activation_request = ArtifactActivationRequest {
            installation_id: predecessor.installation_id,
            scope: ModuleInstallationScope::Platform,
            expected_revision: 1,
            context: lifecycle_context(None),
            reason: "activate initial module release".to_string(),
        };
        let predecessor_activation = store
            .activate_artifact(predecessor_activation_request.clone())
            .await
            .expect("activate predecessor");
        assert_eq!(predecessor_activation.predecessor_installation_id, None);
        assert_eq!(predecessor_activation.installation_revision, 2);
        assert_eq!(
            store
                .activate_artifact(predecessor_activation_request.clone())
                .await
                .expect("idempotent predecessor activation"),
            predecessor_activation
        );
        assert!(matches!(
            store
                .activate_artifact(ArtifactActivationRequest {
                    context: lifecycle_context(None),
                    ..predecessor_activation_request
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(_))
        ));

        let mut incompatible_settings_package = predecessor_package.clone();
        incompatible_settings_package.reference.digest = format!("sha256:{}", "c".repeat(64));
        incompatible_settings_package.descriptor.version = "1.1.0".to_string();
        let settings_schema = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": { "theme": { "type": "string" } },
            "additionalProperties": false,
        });
        let settings_schema_digest = crate::canonical_schema_digest(&settings_schema);
        incompatible_settings_package.descriptor.schema_documents =
            vec![crate::ArtifactSchemaDocument {
                digest: settings_schema_digest.clone(),
                document: settings_schema,
            }];
        incompatible_settings_package
            .descriptor
            .settings_schema_digest = Some(settings_schema_digest);
        let incompatible_settings = ModuleInstaller::new(
            FixtureRegistry(incompatible_settings_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        )
        .admit(admission_command(
            incompatible_settings_package.reference.clone(),
            ModuleInstallationScope::Platform,
        ))
        .await
        .expect("incompatible settings admission");
        assert!(matches!(
            store
                .activate_artifact(ArtifactActivationRequest {
                    installation_id: incompatible_settings.installation_id,
                    scope: ModuleInstallationScope::Platform,
                    expected_revision: 1,
                    context: lifecycle_context(None),
                    reason: "attempt schema-changing activation".to_string(),
                })
                .await,
            Err(ModuleInstallationError::AdmissionRevisionConflict(reason))
                if reason == "activation requires equal artifact settings schemas until a guarded settings migration is available"
        ));
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
        let admitted_successor_pointer = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT previous_installation_id FROM module_artifact_installations \
                 WHERE installation_id = ?1"
                    .to_string(),
                vec![successor.installation_id.to_string().into()],
            ))
            .await
            .expect("admitted successor pointer query")
            .expect("admitted successor pointer row");
        assert_eq!(
            admitted_successor_pointer
                .try_get::<Option<String>>("", "previous_installation_id")
                .expect("admitted successor predecessor pointer"),
            None
        );
        let successor_activation = store
            .activate_artifact(ArtifactActivationRequest {
                installation_id: successor.installation_id,
                scope: ModuleInstallationScope::Platform,
                expected_revision: 1,
                context: lifecycle_context(None),
                reason: "activate upgrade candidate".to_string(),
            })
            .await
            .expect("activate successor");
        assert_eq!(
            successor_activation.predecessor_installation_id,
            Some(predecessor.installation_id)
        );
        assert_eq!(successor_activation.installation_revision, 2);
        assert_eq!(successor_activation.predecessor_revision, Some(3));
        for (table, label) in [
            ("module_transition_checkpoints", "transition checkpoints"),
            ("module_retention_holds", "retention holds"),
        ] {
            let row = database
                .query_one_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("SELECT COUNT(*) AS count FROM {table}"),
                ))
                .await
                .expect("plain activation side-effect query")
                .expect("plain activation side-effect row");
            assert_eq!(
                i64::try_get(&row, "", "count").expect("plain activation side-effect count"),
                0,
                "plain activation must not create {label}"
            );
        }
        let successor_pointer = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT previous_installation_id FROM module_artifact_installations \
                 WHERE installation_id = ?1"
                    .to_string(),
                vec![successor.installation_id.to_string().into()],
            ))
            .await
            .expect("successor pointer query")
            .expect("successor pointer row");
        assert_eq!(
            successor_pointer
                .try_get::<String>("", "previous_installation_id")
                .expect("successor predecessor pointer"),
            predecessor.installation_id.to_string()
        );
        let predecessor_persistence = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT data_owner_id, settings_instance_id FROM module_artifact_installations \
                 WHERE installation_id = ?1"
                    .to_string(),
                vec![predecessor.installation_id.to_string().into()],
            ))
            .await
            .expect("predecessor persistence query")
            .expect("predecessor persistence row");
        let successor_persistence = database
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT data_owner_id, settings_instance_id FROM module_artifact_installations \
                 WHERE installation_id = ?1"
                    .to_string(),
                vec![successor.installation_id.to_string().into()],
            ))
            .await
            .expect("successor persistence query")
            .expect("successor persistence row");
        assert_eq!(
            successor_persistence
                .try_get::<String>("", "data_owner_id")
                .expect("successor data owner"),
            predecessor_persistence
                .try_get::<String>("", "data_owner_id")
                .expect("predecessor data owner")
        );
        assert_eq!(
            successor_persistence
                .try_get::<String>("", "settings_instance_id")
                .expect("successor settings instance"),
            predecessor_persistence
                .try_get::<String>("", "settings_instance_id")
                .expect("predecessor settings instance")
        );

        let request = ArtifactRollbackRequest {
            installation_id: successor.installation_id,
            scope: ModuleInstallationScope::Platform,
            expected_revision: 2,
            context: lifecycle_context(None),
            reason: "restore predecessor after failed upgrade".to_string(),
            target_capability_grant_revision: 7,
        };
        // 1. Verify that irreversible migration prohibits rollback:
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_installations SET has_irreversible_migration = 1 WHERE installation_id = ?1".to_string(),
                vec![successor.installation_id.to_string().into()],
            ))
            .await
            .expect("set irreversible migration flag");

        let irreversible_attempt = store.rollback_artifact(request.clone()).await;

        assert!(matches!(
            irreversible_attempt,
            Err(ModuleInstallationError::AdmissionRevisionConflict(reason))
                if reason == "rollback is prohibited by the recorded data-migration policy"
        ));

        // 2. Clear irreversible migration flag and verify rollback succeeds:
        database
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "UPDATE module_artifact_installations SET has_irreversible_migration = 0 WHERE installation_id = ?1".to_string(),
                vec![successor.installation_id.to_string().into()],
            ))
            .await
            .expect("clear irreversible migration flag");

        let result = store
            .rollback_artifact(request.clone())
            .await
            .expect("rollback");
        assert_eq!(result.source_installation_id, successor.installation_id);
        assert_eq!(result.target_installation_id, predecessor.installation_id);
        assert_eq!(result.source_revision, 3);
        assert_eq!(result.target_revision, 4);
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
            .query_one_raw(Statement::from_string(
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
    async fn test_transition_rollback_operation_resolves_and_recovers_predecessor() {
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
        let initial_package = package(ArtifactPayloadKind::Rhai);
        let mut successor_package = initial_package.clone();
        successor_package.reference.digest = format!("sha256:{}", "7".repeat(64));
        successor_package.descriptor.version = "2.0.0".to_string();

        let store = SeaOrmArtifactInstallationStore::new(database.clone());
        let installer = ModuleInstaller::new(
            FixtureRegistry(initial_package.clone()),
            store.clone(),
            InMemoryArtifactBlobStore::default(),
            trust_verifier(),
            trust_policy(),
            AllowArtifactPermissionRegistrar,
        );
        let predecessor = installer
            .admit(admission_command(
                initial_package.reference.clone(),
                ModuleInstallationScope::Platform,
            ))
            .await
            .expect("predecessor admission");
        store
            .activate_artifact(ArtifactActivationRequest {
                installation_id: predecessor.installation_id,
                scope: ModuleInstallationScope::Platform,
                expected_revision: 1,
                context: lifecycle_context(None),
                reason: "initial activation".to_string(),
            })
            .await
            .expect("activate predecessor");

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

        let activation_result = store
            .activate_artifact(ArtifactActivationRequest {
                installation_id: successor.installation_id,
                scope: ModuleInstallationScope::Platform,
                expected_revision: 1,
                context: lifecycle_context(None),
                reason: "activate successor".to_string(),
            })
            .await
            .expect("activate successor");

        let rollback_result = store
            .rollback_transition_operation(TransitionRollbackRequest {
                operation_id: activation_result.operation_id,
                scope: ModuleInstallationScope::Platform,
                context: lifecycle_context(None),
                reason: "rollback transition via operation id".to_string(),
            })
            .await
            .expect("rollback transition operation succeeds");

        assert_eq!(
            rollback_result.source_installation_id,
            successor.installation_id
        );
        assert_eq!(
            rollback_result.target_installation_id,
            predecessor.installation_id
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
            .execute_raw(Statement::from_string(
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
                context: lifecycle_context(Some(tenant_id)),
                reason: "tenant supersedes this artifact".to_string(),
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
                context: lifecycle_context(Some(tenant_id)),
                reason: "tenant disables the platform artifact".to_string(),
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
