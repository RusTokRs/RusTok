//! Digest-pinned OCI validation and admission into streamed platform-CAS publication.
//!
//! Enforces:
//! - Exact digest-pinned OCI references (rejects mutable tags or unpinned references).
//! - Streamed platform-CAS staging and publication under verified SHA-256 digests.
//! - Durable idempotent release admission intent before CAS mutation.
//! - Immutable admission commit without creating scoped installations, predecessors,
//!   serving selections, routable bindings, schedules, or work generations.
//! - Canonical architectural invariant: runtime and recovery read CAS only and NEVER
//!   fall back to OCI.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ArtifactAdmissionLimits, ArtifactPayloadSource,
    ArtifactRegistry, ControlPlaneInfrastructure, DurableArtifactBlobStore,
    MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, ModuleArtifactDescriptor,
    ModuleCommandContext, ModuleInstallationScope, OciArtifactReference,
    ReleaseAdmissionIntentJournal, ReleaseAdmissionJournalError,
    TrustVerificationRequest, TrustVerifier,
};

/// Error conditions during OCI release admission.
#[derive(Debug, Error)]
pub enum OciReleaseAdmissionError {
    #[error("Invalid OCI reference: {0}")]
    InvalidReference(String),

    #[error("Unpinned OCI reference `{0}`; releases must be pinned by sha256 digest")]
    UnpinnedReference(String),

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
}

/// Command requesting immutable release admission for a digest-pinned OCI artifact.
#[derive(Debug, Clone)]
pub struct OciReleaseAdmissionCommand {
    pub reference: OciArtifactReference,
    pub scope: ModuleInstallationScope,
    pub context: ModuleCommandContext,
    pub trust_policy_revision: Option<u64>,
    pub capability_policy_revision: Option<u64>,
}

/// Immutable admission receipt certifying that an OCI artifact has been validated,
/// staged, and published into platform CAS.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OciAdmissionReceipt {
    pub reference: OciArtifactReference,
    pub descriptor: ModuleArtifactDescriptor,
    pub payload_digest: String,
    pub payload_size_bytes: u64,
    pub cas_published: bool,
    pub admitted_at: DateTime<Utc>,
}

/// Service composing digest-pinned OCI validation, admission intent journaling,
/// and streamed platform-CAS publication.
#[derive(Clone)]
pub struct OciReleaseAdmissionService {
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
    hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit())
}

impl OciReleaseAdmissionService {
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

    /// Admits one digest-pinned OCI artifact into the platform catalog and platform CAS.
    ///
    /// Admission is strictly decoupled from scoped installation: it creates NO
    /// installation record, NO predecessor, NO serving selection, and NO routable
    /// bindings.
    pub async fn admit_release(
        &self,
        command: OciReleaseAdmissionCommand,
    ) -> Result<OciAdmissionReceipt, OciReleaseAdmissionError> {
        // 1. Validate digest-pinned reference
        command
            .reference
            .validate()
            .map_err(|e| OciReleaseAdmissionError::InvalidReference(e.to_string()))?;

        if !valid_sha256_digest(&command.reference.digest) {
            return Err(OciReleaseAdmissionError::UnpinnedReference(
                command.reference.canonical(),
            ));
        }

        let backend = self.db.get_database_backend();
        let (scope_kind, scope_tenant_key) = match &command.scope {
            ModuleInstallationScope::Platform => ("platform", "platform".to_string()),
            ModuleInstallationScope::Tenant { tenant_id } => ("tenant", tenant_id.to_string()),
        };

        // 2. Derive request digest for intent and idempotency tracking
        let request_digest = format!(
            "sha256:{}",
            hex::encode(Sha256::digest(
                format!(
                    "{}:{}:{}",
                    command.reference.canonical(),
                    command.context.actor_id,
                    command.context.idempotency_key
                )
                .as_bytes()
            ))
        );

        // 3. Check if release already admitted under this release digest
        let existing_by_digest = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT release_digest, registry, repository, slug, version, \
                            payload_digest, payload_media_type, payload_size_bytes, \
                            descriptor_json, actor_id, idempotency_key, admitted_at \
                     FROM module_admitted_oci_releases \
                     WHERE release_digest = {}",
                    placeholder(backend, 1)
                ),
                vec![command.reference.digest.clone().into()],
            ))
            .await
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;

        if let Some(row) = existing_by_digest {
            let descriptor_json: String = row
                .try_get("", "descriptor_json")
                .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
            let descriptor: ModuleArtifactDescriptor = serde_json::from_str(&descriptor_json)
                .map_err(|e| OciReleaseAdmissionError::Serialization(e.to_string()))?;
            let payload_size_bytes: i64 = row
                .try_get("", "payload_size_bytes")
                .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
            let admitted_at: DateTime<Utc> = match backend {
                DbBackend::Postgres => row
                    .try_get("", "admitted_at")
                    .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?,
                _ => {
                    let ts_str: String = row
                        .try_get("", "admitted_at")
                        .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
                    DateTime::parse_from_rfc3339(&ts_str)
                        .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?
                        .with_timezone(&Utc)
                }
            };

            // Verify payload is in CAS
            self.blobs
                .get_verified(&descriptor.artifact_digest)
                .await
                .map_err(|e| OciReleaseAdmissionError::CasStorage(e.to_string()))?;

            return Ok(OciAdmissionReceipt {
                reference: command.reference.clone(),
                descriptor: descriptor.clone(),
                payload_digest: descriptor.artifact_digest,
                payload_size_bytes: payload_size_bytes as u64,
                cas_published: false, // already published in previous attempt
                admitted_at,
            });
        }

        // 4. Check idempotency conflict under (scope_kind, scope_tenant_key, actor_id, idempotency_key)
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
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;

        if let Some(row) = existing_by_key {
            let stored_digest: String = row
                .try_get("", "release_digest")
                .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
            if stored_digest != command.reference.digest {
                return Err(OciReleaseAdmissionError::IdempotencyConflict(
                    command.context.idempotency_key,
                    format!(
                        "Idempotency key was already used to admit release `{stored_digest}`, cannot reuse for `{}`",
                        command.reference.digest
                    ),
                ));
            }
        }

        // 5. Reserve staging intent in admission journal before CAS mutation
        ReleaseAdmissionIntentJournal::record_staging_intent(
            &self.db,
            &command.scope,
            &command.context,
            &request_digest,
        )
        .await
        .map_err(|e| match e {
            ReleaseAdmissionJournalError::Conflict(key, msg) => {
                OciReleaseAdmissionError::IdempotencyConflict(key, msg)
            }
            other => OciReleaseAdmissionError::Database(other.to_string()),
        })?;

        // 6. Fetch and validate OCI package via registry adapter
        let package = self
            .registry
            .fetch(&command.reference, self.limits)
            .await
            .map_err(|e| OciReleaseAdmissionError::Registry(e.to_string()))?;

        if package.reference != command.reference {
            return Err(OciReleaseAdmissionError::ManifestDigestMismatch {
                requested: command.reference.canonical(),
                received: package.reference.canonical(),
            });
        }

        package
            .verify(self.limits)
            .await
            .map_err(|e| OciReleaseAdmissionError::InvalidDescriptor(e.to_string()))?;

        // Validate descriptor invariants
        if package.descriptor.schema_version != MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION {
            return Err(OciReleaseAdmissionError::InvalidDescriptor(format!(
                "descriptor schema version must be `{MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION}`, received `{}`",
                package.descriptor.schema_version
            )));
        }

        package
            .descriptor
            .validate()
            .map_err(|e| OciReleaseAdmissionError::InvalidDescriptor(e.to_string()))?;

        let expected_media_type = package.descriptor.payload_kind.oci_layer_media_type();
        if package.media_type != expected_media_type {
            return Err(OciReleaseAdmissionError::InvalidLayer(format!(
                "layer media type mismatch: descriptor declares `{expected_media_type}`, package has `{}`",
                package.media_type
            )));
        }

        // 7. Optional trust policy verification
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
                .map_err(|e| OciReleaseAdmissionError::TrustVerification(e.to_string()))?;

            if !decision.admitted() {
                return Err(OciReleaseAdmissionError::TrustVerification(
                    "trust verification decision rejected artifact admission".to_string(),
                ));
            }
        }

        // 8. Stream payload into platform CAS staging and publish create-if-absent
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
                    .map_err(|e| OciReleaseAdmissionError::CasStorage(e.to_string()))?;

                if let Err(error) = self.blobs.publish(&staged).await {
                    let _ = self.blobs.discard(&staged).await;
                    return Err(OciReleaseAdmissionError::CasStorage(error.to_string()));
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
                    .map_err(|e| OciReleaseAdmissionError::CasStorage(e.to_string()))?;

                let size = staged.size_bytes;
                let publish_result = self.blobs.publish(&staged).await;
                let _ = tokio::fs::remove_file(&path).await;

                if let Err(error) = publish_result {
                    let _ = self.blobs.discard(&staged).await;
                    return Err(OciReleaseAdmissionError::CasStorage(error.to_string()));
                }
                (size, true)
            }
        };

        // 9. Verify published payload in CAS
        self.blobs
            .get_verified(&package.descriptor.artifact_digest)
            .await
            .map_err(|e| OciReleaseAdmissionError::CasStorage(e.to_string()))?;

        let descriptor_json = serde_json::to_string(&package.descriptor)
            .map_err(|e| OciReleaseAdmissionError::Serialization(e.to_string()))?;
        let now = self.infrastructure.now();

        // 10. Commit immutable admission record in database
        self.db
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_admitted_oci_releases (\
                        release_digest, scope_kind, scope_tenant_key, registry, repository, \
                        slug, version, payload_digest, payload_media_type, payload_size_bytes, \
                        descriptor_json, actor_id, idempotency_key, trace_id, correlation_id, admitted_at\
                    ) VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}, {}) \
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
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;

        let payload_digest = package.descriptor.artifact_digest.clone();
        Ok(OciAdmissionReceipt {
            reference: command.reference,
            descriptor: package.descriptor,
            payload_digest,
            payload_size_bytes: payload_size,
            cas_published,
            admitted_at: now,
        })
    }

    /// Looks up an already-admitted OCI release by its exact manifest digest.
    pub async fn get_admitted_release(
        &self,
        release_digest: &str,
    ) -> Result<Option<OciAdmissionReceipt>, OciReleaseAdmissionError> {
        let backend = self.db.get_database_backend();
        let row = self
            .db
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT release_digest, registry, repository, payload_digest, \
                            payload_size_bytes, descriptor_json, admitted_at \
                     FROM module_admitted_oci_releases \
                     WHERE release_digest = {}",
                    placeholder(backend, 1)
                ),
                vec![release_digest.into()],
            ))
            .await
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;

        let Some(row) = row else {
            return Ok(None);
        };

        let registry: String = row
            .try_get("", "registry")
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
        let repository: String = row
            .try_get("", "repository")
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
        let payload_digest: String = row
            .try_get("", "payload_digest")
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
        let payload_size_bytes: i64 = row
            .try_get("", "payload_size_bytes")
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
        let descriptor_json: String = row
            .try_get("", "descriptor_json")
            .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(&descriptor_json)
            .map_err(|e| OciReleaseAdmissionError::Serialization(e.to_string()))?;

        let admitted_at: DateTime<Utc> = match backend {
            DbBackend::Postgres => row
                .try_get("", "admitted_at")
                .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?,
            _ => {
                let ts_str: String = row
                    .try_get("", "admitted_at")
                    .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?;
                DateTime::parse_from_rfc3339(&ts_str)
                    .map_err(|e| OciReleaseAdmissionError::Database(e.to_string()))?
                    .with_timezone(&Utc)
            }
        };

        Ok(Some(OciAdmissionReceipt {
            reference: OciArtifactReference {
                registry,
                repository,
                digest: release_digest.to_string(),
            },
            descriptor,
            payload_digest,
            payload_size_bytes: payload_size_bytes as u64,
            cas_published: false,
            admitted_at,
        }))
    }

    /// Verifies whether the payload bytes for an admitted release exist in platform CAS.
    pub async fn has_cas_payload(&self, payload_digest: &str) -> bool {
        self.blobs.get_verified(payload_digest).await.is_ok()
    }
}
