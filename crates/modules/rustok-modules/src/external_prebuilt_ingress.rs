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
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ArtifactAdmissionLimits, ArtifactPayloadKind, ArtifactPayloadSource,
    ArtifactRegistry, ControlPlaneInfrastructure, DurableArtifactBlobStore,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor,
    ModuleCommandContext, ModuleInstallationScope, OciArtifactReference,
    ReleaseAdmissionIntentJournal, ReleaseAdmissionJournalError,
    TrustVerificationRequest, TrustVerifier,
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
        if !valid_sha256_digest(&self.source_digest) {
            return Err(ExternalPrebuiltIngressError::InvalidLineage(
                format!("lineage source digest `{}` is not a valid sha256 digest", self.source_digest),
            ));
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
        if !valid_sha256_digest(&self.signature_digest) {
            return Err(ExternalPrebuiltIngressError::InvalidSignature(
                format!("signature digest `{}` is not a valid sha256 digest", self.signature_digest),
            ));
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
        if !valid_sha256_digest(&self.sbom_digest) {
            return Err(ExternalPrebuiltIngressError::InvalidSbom(
                format!("SBOM digest `{}` is not a valid sha256 digest", self.sbom_digest),
            ));
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
        if !valid_sha256_digest(&self.provenance_digest) {
            return Err(ExternalPrebuiltIngressError::InvalidProvenance(
                format!("provenance digest `{}` is not a valid sha256 digest", self.provenance_digest),
            ));
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
                    "sidecar payload kind cannot enter dynamic external prebuilt ingress".to_string(),
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
    TrustVerification(String),

    #[error("Platform CAS storage error: {0}")]
    CasStorage(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Idempotency conflict for key `{0}`: {1}")]
    IdempotencyConflict(Uuid, String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("External prebuilt releases cannot enter native promotion")]
    ExternalPrebuiltCannotBePromoted,
}

/// Command requesting admission of an external prebuilt package with independently verified evidence.
#[derive(Debug, Clone)]
pub struct ExternalPrebuiltIngressCommand {
    pub reference: OciArtifactReference,
    pub scope: ModuleInstallationScope,
    pub context: ModuleCommandContext,
    pub evidence: ExternalPrebuiltIngressEvidence,
    pub trust_policy_revision: Option<u64>,
    pub capability_policy_revision: Option<u64>,
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

fn placeholder(backend: DbBackend, idx: usize) -> String {
    match backend {
        DbBackend::Postgres => format!("${idx}"),
        _ => format!("?{idx}"),
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> sea_orm::Value {
    match backend {
        DbBackend::Postgres => sea_orm::Value::Uuid(Some(value)),
        _ => value.to_string().into(),
    }
}

fn valid_sha256_digest(digest: &str) -> bool {
    if !digest.starts_with("sha256:") {
        return false;
    }
    let hex = &digest[7..];
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase())
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
        // 1. Validate digest-pinned reference
        command
            .reference
            .validate()
            .map_err(|e| ExternalPrebuiltIngressError::InvalidReference(e.to_string()))?;

        if !valid_sha256_digest(&command.reference.digest) {
            return Err(ExternalPrebuiltIngressError::UnpinnedReference(
                command.reference.canonical(),
            ));
        }

        // 2. Independently verify all evidence dimensions before CAS or DB mutations
        // Note: failure here rejects admission immediately without mutating quarantine state.
        command.evidence.validate()?;

        let backend = self.db.get_database_backend();
        let (scope_kind, scope_tenant_key) = match &command.scope {
            ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
        };

        // 3. Derive request digest for intent and idempotency tracking
        let request_digest = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(
                format!(
                    "external_prebuilt:{}:{}:{}",
                    command.reference.canonical(),
                    command.context.actor_id,
                    command.context.idempotency_key
                )
                .as_bytes()
            ))
        );

        // 4. Check if release already admitted under this release digest
        let existing_by_digest = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT r.release_digest, r.registry, r.repository, r.slug, r.version, \
                            r.payload_digest, r.payload_size_bytes, r.descriptor_json, \
                            r.artifact_origin, e.native_promotion_denied, e.ingress_at \
                     FROM module_admitted_oci_releases AS r \
                     JOIN module_external_prebuilt_ingress AS e \
                       ON e.release_digest = r.release_digest \
                     WHERE r.release_digest = {}",
                    placeholder(backend, 1)
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
            let payload_size_bytes: i64 = row
                .try_get("", "payload_size_bytes")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            let artifact_origin: String = row
                .try_get("", "artifact_origin")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            let ingress_at: DateTime<Utc> = match backend {
                DbBackend::Postgres => row
                    .try_get("", "ingress_at")
                    .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?,
                _ => {
                    let ts_str: String = row
                        .try_get("", "ingress_at")
                        .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
                    DateTime::parse_from_rfc3339(&ts_str)
                        .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?
                        .with_timezone(&Utc)
                }
            };

            // Verify payload is in CAS
            self.blobs
                .get_verified(&descriptor.artifact_digest)
                .await
                .map_err(|e| ExternalPrebuiltIngressError::CasStorage(e.to_string()))?;

            return Ok(ExternalPrebuiltIngressReceipt {
                reference: command.reference.clone(),
                descriptor: descriptor.clone(),
                payload_digest: descriptor.artifact_digest,
                payload_size_bytes: payload_size_bytes as u64,
                artifact_origin,
                native_promotion_denied: true,
                cas_published: false,
                ingress_at,
            });
        }

        // 5. Check idempotency conflict under (scope_kind, scope_tenant_key, actor_id, idempotency_key)
        let existing_by_key = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT release_digest FROM module_admitted_oci_releases \
                     WHERE scope_kind = {} AND scope_tenant_key = {} \
                       AND actor_id = {} AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    scope_kind.into(),
                    scope_tenant_key.clone().into(),
                    uuid_value(command.context.actor_id, backend),
                    uuid_value(command.context.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        if let Some(row) = existing_by_key {
            let stored_digest: String = row
                .try_get("", "release_digest")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
            if stored_digest != command.reference.digest {
                return Err(ExternalPrebuiltIngressError::IdempotencyConflict(
                    command.context.idempotency_key,
                    format!(
                        "Idempotency key was already used to admit release `{stored_digest}`, cannot reuse for `{}`",
                        command.reference.digest
                    ),
                ));
            }
        }

        // 6. Record staging intent in journal before CAS mutation
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

        // 7. Fetch OCI package from registry and validate against descriptor and evidence
        let package = self
            .registry
            .fetch(&command.reference, self.limits)
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Registry(e.to_string()))?;

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

        // 8. Optional trust policy verification
        if let Some(ref verifier) = self.verifier {
            let req = TrustVerificationRequest {
                reference: package.reference.clone(),
                descriptor: package.descriptor.clone(),
                trust_policy_revision: command.trust_policy_revision.unwrap_or(0),
                capability_policy_revision: command.capability_policy_revision.unwrap_or(0),
            };
            let decision = verifier
                .verify(req)
                .await
                .map_err(|e| ExternalPrebuiltIngressError::TrustVerification(e.to_string()))?;

            if !decision.admitted() {
                return Err(ExternalPrebuiltIngressError::TrustVerification(
                    "trust verification decision rejected external prebuilt admission".to_string(),
                ));
            }
        }

        // 9. Stream payload into platform CAS staging and publish create-if-absent
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
                    let _ = self.blobs.discard(&staged).await;
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
                let _ = tokio::fs::remove_file(&path).await;

                if let Err(error) = publish_result {
                    let _ = self.blobs.discard(&staged).await;
                    return Err(ExternalPrebuiltIngressError::CasStorage(error.to_string()));
                }
                (size, true)
            }
        };

        // 10. Verify published payload in CAS
        self.blobs
            .get_verified(&package.descriptor.artifact_digest)
            .await
            .map_err(|e| ExternalPrebuiltIngressError::CasStorage(e.to_string()))?;

        let descriptor_json = serde_json::to_string(&package.descriptor)
            .map_err(|e| ExternalPrebuiltIngressError::Serialization(e.to_string()))?;
        let now = self.infrastructure.now();

        // 11. Atomically commit admission & external prebuilt evidence in transaction
        let tx = self
            .db
            .begin()
            .await
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_admitted_oci_releases (\
                    release_digest, scope_kind, scope_tenant_key, registry, repository, \
                    slug, version, payload_digest, payload_media_type, payload_size_bytes, \
                    descriptor_json, artifact_origin, actor_id, idempotency_key, trace_id, correlation_id, admitted_at\
                ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, 'external_prebuilt', {}, {}, {}, {}, {}) \
                ON CONFLICT (release_digest) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                placeholder(backend, 9),
                placeholder(backend, 10),
                placeholder(backend, 11),
                placeholder(backend, 12),
                placeholder(backend, 13),
                placeholder(backend, 14),
                placeholder(backend, 15),
                placeholder(backend, 16),
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
                uuid_value(command.context.actor_id, backend),
                uuid_value(command.context.idempotency_key, backend),
                command.context.trace_id.into(),
                uuid_value(command.context.correlation_id, backend),
                match backend {
                    DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                    _ => now.to_rfc3339().into(),
                },
            ],
        ))
        .await
        .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        tx.execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_external_prebuilt_ingress (\
                    release_digest, publisher_identity, lineage_reference, lineage_digest, \
                    signature_reference, signature_digest, sbom_reference, sbom_digest, \
                    provenance_reference, provenance_digest, policy_revision, \
                    license_policy_verified, vulnerability_policy_verified, abi_verified, \
                    capability_verified, native_promotion_denied, ingress_at\
                ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}) \
                ON CONFLICT (release_digest) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                placeholder(backend, 9),
                placeholder(backend, 10),
                placeholder(backend, 11),
                placeholder(backend, 12),
                placeholder(backend, 13),
                placeholder(backend, 14),
                placeholder(backend, 15),
                placeholder(backend, 16),
                placeholder(backend, 17),
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
                    DbBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    _ => 1i32.into(),
                },
                match backend {
                    DbBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    _ => 1i32.into(),
                },
                match backend {
                    DbBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    _ => 1i32.into(),
                },
                match backend {
                    DbBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    _ => 1i32.into(),
                },
                match backend {
                    DbBackend::Postgres => sea_orm::Value::Bool(Some(true)),
                    _ => 1i32.into(),
                },
                match backend {
                    DbBackend::Postgres => sea_orm::Value::ChronoDateTimeUtc(Some(now)),
                    _ => now.to_rfc3339().into(),
                },
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
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT r.release_digest, r.registry, r.repository, r.payload_digest, \
                            r.payload_size_bytes, r.descriptor_json, r.artifact_origin, \
                            e.native_promotion_denied, e.ingress_at \
                     FROM module_admitted_oci_releases AS r \
                     JOIN module_external_prebuilt_ingress AS e \
                       ON e.release_digest = r.release_digest \
                     WHERE r.release_digest = {}",
                    placeholder(backend, 1)
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
        let payload_size_bytes: i64 = row
            .try_get("", "payload_size_bytes")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let descriptor_json: String = row
            .try_get("", "descriptor_json")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(&descriptor_json)
            .map_err(|e| ExternalPrebuiltIngressError::Serialization(e.to_string()))?;
        let artifact_origin: String = row
            .try_get("", "artifact_origin")
            .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;

        let ingress_at: DateTime<Utc> = match backend {
            DbBackend::Postgres => row
                .try_get("", "ingress_at")
                .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?,
            _ => {
                let ts_str: String = row
                    .try_get("", "ingress_at")
                    .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?;
                DateTime::parse_from_rfc3339(&ts_str)
                    .map_err(|e| ExternalPrebuiltIngressError::Database(e.to_string()))?
                    .with_timezone(&Utc)
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
            payload_size_bytes: payload_size_bytes as u64,
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
