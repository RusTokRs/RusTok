//! External-prebuilt dynamic package ingress with independently verified ownership,
//! lineage, signature, SBOM, provenance, ABI/capability, and policy evidence.
//!
//! Architectural invariants:
//! - An external prebuilt enters at an exact OCI manifest digest rather than source-build steps.
//! - Publisher/ownership, lineage, signature, SBOM, provenance, ABI/capability, and policy
//!   evidence must all be independently verified before CAS admission.
//! - Missing or failed evidence rejects the release; rejection alone does NOT mutate quarantine state.
//!   Only a separate authorized security command may quarantine it.
//! - An external prebuilt remains dynamic and CANNOT enter native promotion.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::{
    ArtifactAdmissionLimits, ArtifactPayloadKind, ArtifactPayloadSource, ArtifactRegistry,
    ControlPlaneInfrastructure, DurableArtifactBlobStore,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor, ModuleArtifactPackage,
    ModuleCommandContext, ModuleInstallationScope, OciArtifactReference,
    ReleaseAdmissionIntentJournal,
    ReleaseAdmissionJournalError, TrustVerificationRequest, TrustVerifier, promotion::digest_json,
};

/// Verified publisher/ownership evidence for an external prebuilt artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalPublisherEvidence {
    pub identity: String,
    pub verified: bool,
}

impl ExternalPublisherEvidence {
    pub fn validate(&self) -> Result<(), ExternalPrebuiltIngressError> {
        let trimmed = self.identity.trim();
        if trimmed.is_empty()
            || trimmed != self.identity
            || trimmed.len() > 512
            || trimmed.chars().any(char::is_control)
        {
            return Err(ExternalPrebuiltIngressError::InvalidPublisher(
                "publisher identity must be non-empty, trimmed, and <= 512 chars without control characters".to_string(),
            ));
        }
        if !self.verified {
            return Err(ExternalPrebuiltIngressError::InvalidPublisher(
                "publisher ownership evidence was not verified".to_string(),
            ));
        }
        Ok(())
    }
}

/// Source or reproducible-build lineage evidence for an external prebuilt artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLineageEvidence {
    pub source_reference: String,
    pub source_digest: String,
    pub build_toolchain: String,
    pub verified: bool,
}

impl ExternalLineageEvidence {
    pub fn validate(&self) -> Result<(), ExternalPrebuiltIngressError> {
        let trimmed_ref = self.source_reference.trim();
        if trimmed_ref.is_empty()
            || trimmed_ref != self.source_reference
            || trimmed_ref.len() > 512
            || trimmed_ref.chars().any(char::is_control)
        {
            return Err(ExternalPrebuiltIngressError::InvalidLineage(
                "lineage source reference must be non-empty, trimmed, and <= 512 chars without control characters".to_string(),
            ));
        }
        if !crate::trust::is_sha256_digest(&self.source_digest) {
            return Err(ExternalPrebuiltIngressError::InvalidLineage(format!(
                "lineage source digest `{}` is not a valid sha256 digest",
                self.source_digest
            )));
        }
        let trimmed_toolchain = self.build_toolchain.trim();
        if trimmed_toolchain.is_empty()
            || trimmed_toolchain != self.build_toolchain
            || trimmed_toolchain.len() > 256
            || trimmed_toolchain.chars().any(char::is_control)
        {
            return Err(ExternalPrebuiltIngressError::InvalidLineage(
                "lineage build toolchain must be non-empty, trimmed, and <= 256 chars without control characters".to_string(),
            ));
        }
        if !self.verified {
            return Err(ExternalPrebuiltIngressError::InvalidLineage(
                "lineage evidence was not verified".to_string(),
            ));
        }
        Ok(())
    }
}

/// Cryptographic signature evidence verifying the external prebuilt artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalSignatureEvidence {
    pub signature_reference: String,
    pub signature_digest: String,
    pub verified: bool,
}

impl ExternalSignatureEvidence {
    pub fn validate(&self) -> Result<(), ExternalPrebuiltIngressError> {
        let trimmed_ref = self.signature_reference.trim();
        if trimmed_ref.is_empty()
            || trimmed_ref != self.signature_reference
            || trimmed_ref.len() > 512
            || trimmed_ref.chars().any(char::is_control)
        {
            return Err(ExternalPrebuiltIngressError::InvalidSignature(
                "signature reference must be non-empty, trimmed, and <= 512 chars without control characters".to_string(),
            ));
        }
        if !crate::trust::is_sha256_digest(&self.signature_digest) {
            return Err(ExternalPrebuiltIngressError::InvalidSignature(format!(
                "signature digest `{}` is not a valid sha256 digest",
                self.signature_digest
            )));
        }
        if !self.verified {
            return Err(ExternalPrebuiltIngressError::InvalidSignature(
                "cryptographic signature evidence was not verified".to_string(),
            ));
        }
        Ok(())
    }
}

/// Software Bill of Materials (SBOM) evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalSbomEvidence {
    pub sbom_reference: String,
    pub sbom_digest: String,
    pub media_type: String,
    pub verified: bool,
}

impl ExternalSbomEvidence {
    pub fn validate(&self) -> Result<(), ExternalPrebuiltIngressError> {
        let trimmed_ref = self.sbom_reference.trim();
        if trimmed_ref.is_empty()
            || trimmed_ref != self.sbom_reference
            || trimmed_ref.len() > 512
            || trimmed_ref.chars().any(char::is_control)
        {
            return Err(ExternalPrebuiltIngressError::InvalidSbom(
                "SBOM reference must be non-empty, trimmed, and <= 512 chars without control characters".to_string(),
            ));
        }
        if !crate::trust::is_sha256_digest(&self.sbom_digest) {
            return Err(ExternalPrebuiltIngressError::InvalidSbom(format!(
                "SBOM digest `{}` is not a valid sha256 digest",
                self.sbom_digest
            )));
        }
        if self.media_type.trim().is_empty() || self.media_type.chars().any(char::is_control) {
            return Err(ExternalPrebuiltIngressError::InvalidSbom(
                "SBOM media type must be non-empty without control characters".to_string(),
            ));
        }
        if !self.verified {
            return Err(ExternalPrebuiltIngressError::InvalidSbom(
                "SBOM evidence was not verified".to_string(),
            ));
        }
        Ok(())
    }
}

/// In-toto provenance evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalProvenanceEvidence {
    pub provenance_reference: String,
    pub provenance_digest: String,
    pub media_type: String,
    pub verified: bool,
}

impl ExternalProvenanceEvidence {
    pub fn validate(&self) -> Result<(), ExternalPrebuiltIngressError> {
        let trimmed_ref = self.provenance_reference.trim();
        if trimmed_ref.is_empty()
            || trimmed_ref != self.provenance_reference
            || trimmed_ref.len() > 512
            || trimmed_ref.chars().any(char::is_control)
        {
            return Err(ExternalPrebuiltIngressError::InvalidProvenance(
                "provenance reference must be non-empty, trimmed, and <= 512 chars without control characters".to_string(),
            ));
        }
        if !crate::trust::is_sha256_digest(&self.provenance_digest) {
            return Err(ExternalPrebuiltIngressError::InvalidProvenance(format!(
                "provenance digest `{}` is not a valid sha256 digest",
                self.provenance_digest
            )));
        }
        if self.media_type.trim().is_empty() || self.media_type.chars().any(char::is_control) {
            return Err(ExternalPrebuiltIngressError::InvalidProvenance(
                "provenance media type must be non-empty without control characters".to_string(),
            ));
        }
        if !self.verified {
            return Err(ExternalPrebuiltIngressError::InvalidProvenance(
                "provenance evidence was not verified".to_string(),
            ));
        }
        Ok(())
    }
}

/// ABI and capability verification evidence for external prebuilts.
///
/// External prebuilts MUST remain dynamic (e.g. WasmComponent or RhaiWorkspace)
/// and CANNOT declare native/static execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalAbiCapabilityEvidence {
    pub abi_kind: ArtifactPayloadKind,
    pub declared_capabilities: Vec<String>,
    pub broker_routes_verified: bool,
    pub verified: bool,
}

impl ExternalAbiCapabilityEvidence {
    pub fn validate(&self) -> Result<(), ExternalPrebuiltIngressError> {
        // Strict invariant: external prebuilts MUST be dynamic!
        match self.abi_kind {
            ArtifactPayloadKind::WasmComponent | ArtifactPayloadKind::Rhai => {}
            ArtifactPayloadKind::StaticPromoted => {
                return Err(ExternalPrebuiltIngressError::ExternalPrebuiltCannotBePromoted);
            }
            ArtifactPayloadKind::Sidecar => {
                return Err(ExternalPrebuiltIngressError::InvalidAbiCapability(
                    "sidecar payload kind cannot enter dynamic external prebuilt ingress"
                        .to_string(),
                ));
            }
        }
        if !self.broker_routes_verified {
            return Err(ExternalPrebuiltIngressError::InvalidAbiCapability(
                "broker routes for declared capabilities were not verified".to_string(),
            ));
        }
        if !self.verified {
            return Err(ExternalPrebuiltIngressError::InvalidAbiCapability(
                "ABI and capability evidence was not verified".to_string(),
            ));
        }
        Ok(())
    }
}

/// License, vulnerability, and policy admission evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalPolicyEvidence {
    pub policy_revision: u64,
    pub license_approved: bool,
    pub vulnerability_scan_passed: bool,
    pub verified: bool,
}

impl ExternalPolicyEvidence {
    pub fn validate(&self) -> Result<(), ExternalPrebuiltIngressError> {
        if !self.license_approved {
            return Err(ExternalPrebuiltIngressError::PolicyViolation(
                "license policy check failed or was not approved".to_string(),
            ));
        }
        if !self.vulnerability_scan_passed {
            return Err(ExternalPrebuiltIngressError::PolicyViolation(
                "vulnerability scan check failed or was not passed".to_string(),
            ));
        }
        if !self.verified {
            return Err(ExternalPrebuiltIngressError::PolicyViolation(
                "policy verification evidence was not verified".to_string(),
            ));
        }
        Ok(())
    }
}

/// Complete evidence packet required for external prebuilt ingress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalPrebuiltIngressEvidence {
    pub publisher: ExternalPublisherEvidence,
    pub lineage: ExternalLineageEvidence,
    pub signature: ExternalSignatureEvidence,
    pub sbom: ExternalSbomEvidence,
    pub provenance: ExternalProvenanceEvidence,
    pub abi_capability: ExternalAbiCapabilityEvidence,
    pub policy: ExternalPolicyEvidence,
}

impl ExternalPrebuiltIngressEvidence {
    pub fn validate(&self) -> Result<(), ExternalPrebuiltIngressError> {
        self.publisher.validate()?;
        self.lineage.validate()?;
        self.signature.validate()?;
        self.sbom.validate()?;
        self.provenance.validate()?;
        self.abi_capability.validate()?;
        self.policy.validate()?;
        Ok(())
    }
}

/// Error conditions during external prebuilt ingress.
#[derive(Debug, Error)]
pub enum ExternalPrebuiltIngressError {
    #[error("Invalid OCI reference: {0}")]
    InvalidReference(String),

    #[error("Unpinned OCI reference `{0}`; releases must be pinned by sha256 digest")]
    UnpinnedReference(String),

    #[error("Publisher ownership verification failed: {0}")]
    InvalidPublisher(String),

    #[error("Lineage verification failed: {0}")]
    InvalidLineage(String),

    #[error("Signature verification failed: {0}")]
    InvalidSignature(String),

    #[error("SBOM verification failed: {0}")]
    InvalidSbom(String),

    #[error("Provenance verification failed: {0}")]
    InvalidProvenance(String),

    #[error("ABI or capability verification failed: {0}")]
    InvalidAbiCapability(String),

    #[error("Policy verification failed: {0}")]
    PolicyViolation(String),

    #[error("Registry error: {0}")]
    Registry(String),

    #[error("Manifest digest mismatch: requested `{requested}`, received `{received}`")]
    ManifestDigestMismatch { requested: String, received: String },

    #[error("Descriptor validation failed: {0}")]
    InvalidDescriptor(String),

    #[error("Layer validation failed: {0}")]
    InvalidLayer(String),

    #[error("Trust verification failed: {0}")]
    TrustVerification(#[from] crate::TrustVerificationError),

    #[error("Trust verification decision rejected external prebuilt admission")]
    VerificationRejected,

    #[error("External prebuilt admission requires non-zero trust and capability policy revisions")]
    MissingPolicyRevision,

    #[error("Platform CAS storage error: {0}")]
    CasStorage(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Idempotency conflict for key `{0}`: {1}")]
    IdempotencyConflict(Uuid, String),

    #[error("External prebuilt ingress command context is invalid for its requested scope")]
    InvalidCommandContext,

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("External prebuilt releases cannot enter native promotion")]
    ExternalPrebuiltCannotBePromoted,
}

/// Command requesting admission of an external prebuilt package with independently verified evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalPrebuiltIngressCommand {
    pub reference: OciArtifactReference,
    pub scope: ModuleInstallationScope,
    pub context: ModuleCommandContext,
    pub evidence: ExternalPrebuiltIngressEvidence,
    pub trust_policy_revision: u64,
    pub capability_policy_revision: u64,
}

impl ExternalPrebuiltIngressCommand {
    /// Digest over only the semantic external-prebuilt admission request,
    /// including independently verified evidence and policy revisions.
    /// Replay is already partitioned by scope, actor, and idempotency key;
    /// trace and correlation evidence may change between attempts.
    fn semantic_request_digest(&self) -> Result<String, ExternalPrebuiltIngressError> {
        #[derive(Serialize)]
        struct Fingerprint<'a> {
            reference: &'a OciArtifactReference,
            scope: &'a ModuleInstallationScope,
            evidence: &'a ExternalPrebuiltIngressEvidence,
            trust_policy_revision: u64,
            capability_policy_revision: u64,
        }

        digest_json(&Fingerprint {
            reference: &self.reference,
            scope: &self.scope,
            evidence: &self.evidence,
            trust_policy_revision: self.trust_policy_revision,
            capability_policy_revision: self.capability_policy_revision,
        })
        .map_err(|error| ExternalPrebuiltIngressError::Serialization(error.to_string()))
    }
}

/// Immutable admission receipt certifying that an external prebuilt artifact has been
/// independently verified across all dimensions, staged, and published into platform CAS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalPrebuiltIngressReceipt {
    pub reference: OciArtifactReference,
    pub descriptor: ModuleArtifactDescriptor,
    pub payload_digest: String,
    pub payload_size_bytes: u64,
    pub artifact_origin: String,
    pub native_promotion_denied: bool,
    pub cas_published: bool,
    pub ingress_at: DateTime<Utc>,
}

/// Service executing external prebuilt ingress with independent evidence validation,
/// atomic CAS staging and publication, and strict native promotion denial.
#[derive(Clone)]
pub struct ExternalPrebuiltIngressService {
    db: DatabaseConnection,
    blobs: Arc<dyn DurableArtifactBlobStore>,
    registry: Arc<dyn ArtifactRegistry>,
    verifier: Option<Arc<dyn TrustVerifier>>,
    limits: ArtifactAdmissionLimits,
    infrastructure: ControlPlaneInfrastructure,
}

impl ExternalPrebuiltIngressService {
    pub fn new(
        db: DatabaseConnection,
        blobs: Arc<dyn DurableArtifactBlobStore>,
        registry: Arc<dyn ArtifactRegistry>,
    ) -> Self {
        Self {
            db,
            blobs,
            registry,
            verifier: None,
            limits: ArtifactAdmissionLimits::default(),
            infrastructure: ControlPlaneInfrastructure::default(),
        }
    }

    pub fn with_verifier(mut self, verifier: Arc<dyn TrustVerifier>) -> Self {
        self.verifier = Some(verifier);
        self
    }

    pub fn with_limits(mut self, limits: ArtifactAdmissionLimits) -> Self {
        self.limits = limits;
        self
    }

    pub fn with_infrastructure(mut self, infrastructure: ControlPlaneInfrastructure) -> Self {
        self.infrastructure = infrastructure;
        self
    }

    /// Runs the configured trust verifier, when present, and requires the
    /// decision to be admitted under exactly the policy revisions selected
    /// by the command. Applied on both the fresh-admission and
    /// already-admitted paths so a previously admitted release cannot be
    /// reused under a policy it was never evaluated against.
    async fn verify_trust_policy(
        &self,
        reference: &OciArtifactReference,
        descriptor: &ModuleArtifactDescriptor,
        command: &ExternalPrebuiltIngressCommand,
    ) -> Result<(), ExternalPrebuiltIngressError> {
        if let Some(verifier) = &self.verifier {
            let request = TrustVerificationRequest {
                reference: reference.clone(),
                descriptor: descriptor.clone(),
                trust_policy_revision: command.trust_policy_revision,
                capability_policy_revision: command.capability_policy_revision,
                expected_alloy_workspace_provenance: None,
            };
            let decision = verifier
                .verify(request.clone())
                .await
                .map_err(ExternalPrebuiltIngressError::TrustVerification)?;
            if !decision.admitted_for(&request) {
                return Err(ExternalPrebuiltIngressError::VerificationRejected);
            }
        }
        Ok(())
    }

    /// Admits an external prebuilt package into platform CAS and the platform catalog
    /// after independently verifying all required evidence.
    ///
    /// If evidence is missing, invalid, or unverified, admission is rejected immediately.
    /// Rejection alone DOES NOT mutate quarantine state.
    /// An admitted external prebuilt release has `artifact_origin = 'external_prebuilt'`
    /// and `native_promotion_denied = true`.
    pub async fn admit_external_prebuilt(
        &self,
        command: ExternalPrebuiltIngressCommand,
    ) -> Result<ExternalPrebuiltIngressReceipt, ExternalPrebuiltIngressError> {
        // 1. Validate the complete owner command before any registry, CAS, or
        // database action. The context must identify exactly the scope it can
        // mutate.
        if !command.scope.matches_command_context(&command.context) {
            return Err(ExternalPrebuiltIngressError::InvalidCommandContext);
        }
        if command.trust_policy_revision == 0 || command.capability_policy_revision == 0 {
            return Err(ExternalPrebuiltIngressError::MissingPolicyRevision);
        }

        // 2. Validate digest-pinned reference
        command
            .reference
            .validate()
            .map_err(|e| ExternalPrebuiltIngressError::InvalidReference(e.to_string()))?;

        if !crate::trust::is_sha256_digest(&command.reference.digest) {
            return Err(ExternalPrebuiltIngressError::UnpinnedReference(
                command.reference.canonical(),
            ));
        }

        // 3. Independently verify all evidence dimensions before CAS or DB mutations
        // Note: failure here rejects admission immediately without mutating quarantine state.
        command.evidence.validate()?;

        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let (scope_kind, scope_tenant_key) = match &command.scope {
            ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
        };

        // 4. Bind every semantic command fact, including independently
        // verified evidence, into one durable replay fingerprint.
        // Observability context is stored as evidence, not replay identity.
        let request_digest = command.semantic_request_digest()?;

        // 5. Reject a reused owner idempotency key before the global immutable
        // release lookup can make a changed command appear successful.
        let existing_by_key = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT release_digest, request_digest FROM module_admitted_oci_releases \
                     WHERE scope_kind = {} AND scope_tenant_key = {} \
                       AND actor_id = {} AND idempotency_key = {}",
                    backend.placeholder(1),
                    backend.placeholder(2),
                    backend.placeholder(3),
                    backend.placeholder(4),
                ),
                vec![
                    scope_kind.into(),
                    scope_tenant_key.clone().into(),
                    backend.uuid_value(command.context.actor_id),
                    backend.uuid_value(command.context.idempotency_key),
                ],
            ))
            .await
            .map_err(|error| ExternalPrebuiltIngressError::Database(error.to_string()))?;

        if let Some(row) = existing_by_key {
            let stored_digest: String = row
                .try_get("", "release_digest")
                .map_err(|error| ExternalPrebuiltIngressError::Database(error.to_string()))?;
            let stored_request_digest: String = row
                .try_get("", "request_digest")
                .map_err(|error| ExternalPrebuiltIngressError::Database(error.to_string()))?;
            if stored_request_digest != request_digest {
                return Err(ExternalPrebuiltIngressError::IdempotencyConflict(
                    command.context.idempotency_key,
                    format!(
                        "Idempotency key was already used to admit release `{stored_digest}` with different command evidence"
                    ),
                ));
            }
        }

        // 6. Check if release already admitted under this release digest.
        let existing_by_digest = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT r.release_digest, r.registry, r.repository, r.slug, r.version, \
                            r.payload_digest, r.payload_size_bytes, r.descriptor_json, \
                            r.artifact_origin, e.native_promotion_denied, e.ingress_at \
                     FROM module_admitted_oci_releases AS r \
                     JOIN module_external_prebuilt_ingress AS e \
                       ON e.release_digest = r.release_digest \
                     WHERE r.release_digest = {}",
                    backend.placeholder(1)
                ),
                vec![command.reference.digest.clone().into()],
            ))
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        if let Some(row) = existing_by_digest {
            let descriptor_json: String = row
                .try_get("", "descriptor_json")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            let descriptor: ModuleArtifactDescriptor = serde_json::from_str(&descriptor_json)
                .map_err(|e| ExternalPrebuiltIngressError::Serialization(e.to_string()))?;
            descriptor
                .validate()
                .map_err(|e| ExternalPrebuiltIngressError::InvalidDescriptor(e.to_string()))?;
            let stored_slug: String = row
                .try_get("", "slug")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            let stored_version: String = row
                .try_get("", "version")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            let stored_payload_digest: String = row
                .try_get("", "payload_digest")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            if descriptor.slug != stored_slug
                || descriptor.version != stored_version
                || descriptor.artifact_digest != stored_payload_digest
            {
                return Err(ExternalPrebuiltIngressError::InvalidDescriptor(
                    "stored descriptor does not match the admitted release identity"
                        .to_string(),
                ));
            }
            let payload_size_bytes =
                crate::backend::nonnegative_count_from_row(&row, "payload_size_bytes")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            let artifact_origin: String = row
                .try_get("", "artifact_origin")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            let ingress_at: DateTime<Utc> = match backend {
                SupportedBackend::Postgres => row
                    .try_get("", "ingress_at")
                    .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?,
                SupportedBackend::Sqlite => {
                    let ts_str: String = row
                        .try_get("", "ingress_at")
                        .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
                    crate::backend::parse_sqlite_datetime(&ts_str)
                        .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?
                }
            };

            // Verify payload is in CAS
            self.blobs
                .get_verified(&descriptor.artifact_digest)
                .await
                .map_err(|e| ExternalPrebuiltIngressError::CasStorage(e.to_string()))?;

            // The stored admission predates this command and records no
            // policy basis; re-verify against the revisions this command
            // selected before certifying the release to it.
            self.verify_trust_policy(&command.reference, &descriptor, &command)
                .await?;

            return Ok(ExternalPrebuiltIngressReceipt {
                reference: command.reference.clone(),
                descriptor: descriptor.clone(),
                payload_digest: descriptor.artifact_digest,
                payload_size_bytes,
                artifact_origin,
                native_promotion_denied: true,
                cas_published: false,
                ingress_at,
            });
        }

        // 7. Record staging intent in journal before CAS mutation.
        ReleaseAdmissionIntentJournal::record_staging_intent(
            &self.db,
            &command.scope,
            &command.context,
            &request_digest,
        )
        .await
        .map_err(|e| match e {
            ReleaseAdmissionJournalError::Conflict(key, msg) => {
                ExternalPrebuiltIngressError::IdempotencyConflict(key, msg)
            }
            other => ExternalPrebuiltIngressError::Database(other.to_string()),
        })?;

        // 8. Fetch OCI package from registry and validate against descriptor and evidence
        let package = self
            .registry
            .fetch(&command.reference, self.limits)
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Registry(e.to_string()))?;
        let temporary_payload_path = match &package.payload {
            ArtifactPayloadSource::TemporaryFile(path) => Some(path.clone()),
            ArtifactPayloadSource::Bytes(_) => None,
        };
        let result = self
            .admit_fetched_external_prebuilt(
                command,
                backend,
                scope_kind,
                scope_tenant_key,
                request_digest,
                package,
            )
            .await;
        if let Some(path) = temporary_payload_path {
            crate::remove_staged_file_best_effort(&path).await;
        }
        result
    }

    async fn admit_fetched_external_prebuilt(
        &self,
        command: ExternalPrebuiltIngressCommand,
        backend: SupportedBackend,
        scope_kind: &'static str,
        scope_tenant_key: String,
        request_digest: String,
        package: ModuleArtifactPackage,
    ) -> Result<ExternalPrebuiltIngressReceipt, ExternalPrebuiltIngressError> {
        if package.reference != command.reference {
            return Err(ExternalPrebuiltIngressError::ManifestDigestMismatch {
                requested: command.reference.canonical(),
                received: package.reference.canonical(),
            });
        }

        package
            .verify(self.limits)
            .await
            .map_err(|e| ExternalPrebuiltIngressError::InvalidDescriptor(e.to_string()))?;

        if package.descriptor.schema_version != MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION {
            return Err(ExternalPrebuiltIngressError::InvalidDescriptor(format!(
                "descriptor schema version must be `{MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION}`, received `{}`",
                package.descriptor.schema_version
            )));
        }

        package
            .descriptor
            .validate()
            .map_err(|e| ExternalPrebuiltIngressError::InvalidDescriptor(e.to_string()))?;

        // Verify payload kind matches declared dynamic ABI in evidence
        if package.descriptor.payload_kind != command.evidence.abi_capability.abi_kind {
            return Err(ExternalPrebuiltIngressError::InvalidAbiCapability(format!(
                "descriptor payload kind `{:?}` does not match verified evidence ABI kind `{:?}`",
                package.descriptor.payload_kind, command.evidence.abi_capability.abi_kind
            )));
        }

        let expected_media_type = package.descriptor.payload_kind.oci_layer_media_type();
        if package.media_type != expected_media_type {
            return Err(ExternalPrebuiltIngressError::InvalidLayer(format!(
                "layer media type mismatch: descriptor declares `{expected_media_type}`, package has `{}`",
                package.media_type
            )));
        }

        // 9. Trust policy verification under the revisions selected by the command
        self.verify_trust_policy(&package.reference, &package.descriptor, &command)
            .await?;

        // 10. Stream payload into platform CAS staging and publish create-if-absent
        let (payload_size, cas_published) = match package.payload {
            ArtifactPayloadSource::Bytes(bytes) => {
                let size = bytes.len() as u64;
                let staged = self
                    .blobs
                    .stage(
                        &package.descriptor.artifact_digest,
                        &package.media_type,
                        &bytes,
                    )
                    .await
                    .map_err(|e| ExternalPrebuiltIngressError::CasStorage(e.to_string()))?;

                if let Err(error) = self.blobs.publish(&staged).await {
                    self.blobs.discard_best_effort(&staged).await;
                    return Err(ExternalPrebuiltIngressError::CasStorage(error.to_string()));
                }
                (size, true)
            }
            ArtifactPayloadSource::TemporaryFile(path) => {
                let staged = self
                    .blobs
                    .stage_file(
                        &package.descriptor.artifact_digest,
                        &package.media_type,
                        &path,
                    )
                    .await
                    .map_err(|e| ExternalPrebuiltIngressError::CasStorage(e.to_string()))?;

                let size = staged.size_bytes;
                let publish_result = self.blobs.publish(&staged).await;
                crate::remove_staged_file_best_effort(&path).await;

                if let Err(error) = publish_result {
                    self.blobs.discard_best_effort(&staged).await;
                    return Err(ExternalPrebuiltIngressError::CasStorage(error.to_string()));
                }
                (size, true)
            }
        };

        // 11. Verify published payload in CAS
        self.blobs
            .get_verified(&package.descriptor.artifact_digest)
            .await
            .map_err(|e| ExternalPrebuiltIngressError::CasStorage(e.to_string()))?;

        let descriptor_json = serde_json::to_string(&package.descriptor)
            .map_err(|e| ExternalPrebuiltIngressError::Serialization(e.to_string()))?;
        let now = self.infrastructure.now();

        // 12. Atomically commit admission & external prebuilt evidence in transaction
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        crate::backend::set_platform_scope(&tx)
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        tx.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_admitted_oci_releases (\
                    release_digest, scope_kind, scope_tenant_key, registry, repository, \
                    slug, version, payload_digest, payload_media_type, payload_size_bytes, \
                    descriptor_json, artifact_origin, actor_id, idempotency_key, trace_id, correlation_id, \
                    request_digest, admitted_at\
                ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'external_prebuilt', {}, {}, {}, {}, {}, {}) \
                ON CONFLICT (release_digest) DO NOTHING",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
                backend.placeholder(6),
                backend.placeholder(7),
                backend.placeholder(8),
                backend.placeholder(9),
                backend.placeholder(10),
                backend.placeholder(11),
                backend.placeholder(12),
                backend.placeholder(13),
                backend.placeholder(14),
                backend.placeholder(15),
                backend.placeholder(16),
                backend.placeholder(17),
            ),
            vec![
                command.reference.digest.clone().into(),
                scope_kind.into(),
                scope_tenant_key.into(),
                command.reference.registry.clone().into(),
                command.reference.repository.clone().into(),
                package.descriptor.slug.clone().into(),
                package.descriptor.version.clone().into(),
                package.descriptor.artifact_digest.clone().into(),
                package.media_type.clone().into(),
                (payload_size as i64).into(),
                descriptor_json.into(),
                backend.uuid_value(command.context.actor_id),
                backend.uuid_value(command.context.idempotency_key),
                command.context.trace_id.into(),
                backend.uuid_value(command.context.correlation_id),
                request_digest.into(),
                backend.datetime_value(now),
            ],
        ))
        .await
        .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        tx.execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_external_prebuilt_ingress (\
                    release_digest, publisher_identity, lineage_reference, lineage_digest, \
                    signature_reference, signature_digest, sbom_reference, sbom_digest, \
                    provenance_reference, provenance_digest, policy_revision, \
                    license_policy_verified, vulnerability_policy_verified, abi_verified, \
                    capability_verified, native_promotion_denied, ingress_at\
                ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}) \
                ON CONFLICT (release_digest) DO NOTHING",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
                backend.placeholder(6),
                backend.placeholder(7),
                backend.placeholder(8),
                backend.placeholder(9),
                backend.placeholder(10),
                backend.placeholder(11),
                backend.placeholder(12),
                backend.placeholder(13),
                backend.placeholder(14),
                backend.placeholder(15),
                backend.placeholder(16),
                backend.placeholder(17),
            ),
            vec![
                command.reference.digest.clone().into(),
                command.evidence.publisher.identity.into(),
                command.evidence.lineage.source_reference.into(),
                command.evidence.lineage.source_digest.into(),
                command.evidence.signature.signature_reference.into(),
                command.evidence.signature.signature_digest.into(),
                command.evidence.sbom.sbom_reference.into(),
                command.evidence.sbom.sbom_digest.into(),
                command.evidence.provenance.provenance_reference.into(),
                command.evidence.provenance.provenance_digest.into(),
                (command.evidence.policy.policy_revision as i64).into(),
                match backend {
                    SupportedBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    SupportedBackend::Sqlite => 1i32.into(),
                },
                match backend {
                    SupportedBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    SupportedBackend::Sqlite => 1i32.into(),
                },
                match backend {
                    SupportedBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    SupportedBackend::Sqlite => 1i32.into(),
                },
                match backend {
                    SupportedBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    SupportedBackend::Sqlite => 1i32.into(),
                },
                match backend {
                    SupportedBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    SupportedBackend::Sqlite => 1i32.into(),
                },
                backend.datetime_value(now),
            ],
        ))
        .await
        .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        let payload_digest = package.descriptor.artifact_digest.clone();
        Ok(ExternalPrebuiltIngressReceipt {
            reference: command.reference,
            descriptor: package.descriptor,
            payload_digest,
            payload_size_bytes: payload_size,
            artifact_origin: "external_prebuilt".to_string(),
            native_promotion_denied: true,
            cas_published,
            ingress_at: now,
        })
    }

    /// Looks up an already-admitted external prebuilt package by its exact manifest digest.
    pub async fn get_ingress_record(
        &self,
        release_digest: &str,
    ) -> Result<Option<ExternalPrebuiltIngressReceipt>, ExternalPrebuiltIngressError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend.db_backend(),
                format!(
                    "SELECT r.release_digest, r.registry, r.repository, r.slug, r.version, r.payload_digest, \
                            r.payload_size_bytes, r.descriptor_json, r.artifact_origin, \
                            e.native_promotion_denied, e.ingress_at \
                     FROM module_admitted_oci_releases AS r \
                     JOIN module_external_prebuilt_ingress AS e \
                       ON e.release_digest = r.release_digest \
                     WHERE r.release_digest = {}",
                    backend.placeholder(1)
                ),
                vec![release_digest.into()],
            ))
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let registry: String = row
            .try_get("", "registry")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let repository: String = row
            .try_get("", "repository")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let payload_digest: String = row
            .try_get("", "payload_digest")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let payload_size_bytes =
            crate::backend::nonnegative_count_from_row(&row, "payload_size_bytes")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let descriptor_json: String = row
            .try_get("", "descriptor_json")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(&descriptor_json)
            .map_err(|e| ExternalPrebuiltIngressError::Serialization(e.to_string()))?;
        descriptor
            .validate()
            .map_err(|e| ExternalPrebuiltIngressError::InvalidDescriptor(e.to_string()))?;
        let stored_slug: String = row
            .try_get("", "slug")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let stored_version: String = row
            .try_get("", "version")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        if descriptor.slug != stored_slug
            || descriptor.version != stored_version
            || descriptor.artifact_digest != payload_digest
        {
            return Err(ExternalPrebuiltIngressError::InvalidDescriptor(
                "stored descriptor does not match the admitted release identity".to_string(),
            ));
        }
        let artifact_origin: String = row
            .try_get("", "artifact_origin")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        let ingress_at: DateTime<Utc> = match backend {
            SupportedBackend::Postgres => row
                .try_get("", "ingress_at")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?,
            SupportedBackend::Sqlite => {
                let ts_str: String = row
                    .try_get("", "ingress_at")
                    .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
                crate::backend::parse_sqlite_datetime(&ts_str)
                    .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?
            }
        };

        Ok(Some(ExternalPrebuiltIngressReceipt {
            reference: OciArtifactReference {
                registry,
                repository,
                digest: release_digest.to_string(),
            },
            descriptor,
            payload_digest,
            payload_size_bytes,
            artifact_origin,
            native_promotion_denied: true,
            cas_published: false,
            ingress_at,
        }))
    }

    /// Verifies whether the payload bytes for an admitted release exist in platform CAS.
    pub async fn has_cas_payload(&self, payload_digest: &str) -> bool {
        self.blobs.get_verified(payload_digest).await.is_ok()
    }
}
