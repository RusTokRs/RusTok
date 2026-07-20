//! Transport-neutral protocol for isolated untrusted module builds.
//!
//! The control plane owns immutable request/result evidence. A separately
//! deployed worker owns source materialization and command execution; neither
//! `apps/server` nor the runtime sandbox may implement this port in production.

use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use semver::Version;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::OutboxTransport;

use crate::OciArtifactReference;

use crate::{
    data::{configure_tenant_scope, now_expression, placeholder, uuid_from_row, uuid_value},
    ModuleCommandContext,
};

const MAX_BUILD_REFERENCE_BYTES: usize = 512;
const MAX_BUILD_TEXT_BYTES: usize = 256;
const MAX_BUILD_VERSION_BYTES: usize = 128;
const MAX_ALLOWED_REGISTRIES: usize = 16;
const MAX_SCOPED_ENDPOINTS: usize = 16;
const MAX_VALIDATION_PROFILES: usize = 8;
const MAX_CPU_CORES: u16 = 64;
const MAX_MEMORY_BYTES: u64 = 64 * 1024 * 1024 * 1024;
const MAX_DISK_BYTES: u64 = 100 * 1024 * 1024 * 1024;
const MAX_PROCESSES: u16 = 1_024;
const MAX_OUTPUT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_BUILD_DIAGNOSTICS: usize = 32;
const MAX_WALL_CLOCK_MS: u64 = 2 * 60 * 60 * 1_000;

/// Current transport contract version for isolated module build workers.
pub const MODULE_BUILD_PROTOCOL_VERSION: u32 = 7;

/// Immutable request submitted by the control plane to an isolated worker.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildRequest {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub context: ModuleCommandContext,
    pub project_id: String,
    pub source: ModuleBuildSource,
    pub expected_module_slug: String,
    pub expected_version: String,
    pub runtime_abi: String,
    pub wit: ModuleBuildWitContract,
    pub toolchain: ModuleBuildToolchain,
    pub authoring: ModuleBuildAuthoring,
    pub dependency_policy: ModuleBuildDependencyPolicy,
    pub limits: ModuleBuildLimits,
    pub network_policy: ModuleBuildNetworkPolicy,
    pub validation_profiles: Vec<ModuleBuildValidationProfile>,
    pub attempt: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildSource {
    pub reference: String,
    pub digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildWitContract {
    pub world: String,
    pub version: String,
}

impl ModuleBuildWitContract {
    /// Digest of the exact WIT contract the worker must inspect for protocol v1.
    pub fn protocol_digest(&self) -> String {
        protocol_digest("rustok.module.build.wit.v1", &[&self.world, &self.version])
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildToolchain {
    pub rust_toolchain: String,
    pub component_target: String,
}

impl ModuleBuildToolchain {
    /// Digest of the exact toolchain selection the worker must use for protocol v1.
    pub fn protocol_digest(&self) -> String {
        protocol_digest(
            "rustok.module.build.toolchain.v1",
            &[&self.rust_toolchain, &self.component_target],
        )
    }
}

/// Independently versioned authoring inputs bound to build provenance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildAuthoring {
    pub sdk_version: String,
    pub template_version: String,
}

impl ModuleBuildAuthoring {
    fn validate(&self) -> Result<(), ModuleBuildProtocolError> {
        for version in [&self.sdk_version, &self.template_version] {
            if version.len() > MAX_BUILD_VERSION_BYTES || Version::parse(version).is_err() {
                return Err(ModuleBuildProtocolError::InvalidRequest);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildDependencyPolicy {
    pub lock_digest: String,
    pub allowed_registries: Vec<String>,
    pub allow_git_dependencies: bool,
    pub allow_build_scripts: bool,
    pub allow_native_links: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildLimits {
    pub cpu_cores: u16,
    pub memory_bytes: u64,
    pub disk_bytes: u64,
    pub process_limit: u16,
    pub output_bytes: u64,
    pub wall_clock_ms: u64,
}

/// Network may be used only for a reviewed dependency-materialization phase.
/// Compilation, tests, inspection, and publication remain network-denied.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBuildNetworkPolicy {
    Denied,
    ScopedDependencyMaterialization { endpoints: Vec<String> },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBuildValidationProfile {
    Format,
    Check,
    Lint,
    Test,
    DependencyPolicy,
    Vulnerability,
}

/// Terminal outcome for one requested, image-owned validation profile.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBuildValidationOutcome {
    Passed,
    Failed,
}

/// Machine-readable outcome for one requested validation profile. Raw command
/// output remains behind the separately authorized evidence references.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildValidationResult {
    pub profile: ModuleBuildValidationProfile,
    pub outcome: ModuleBuildValidationOutcome,
}

/// Terminal worker result. Payload, SBOM, provenance, and logs are immutable
/// references, never inline bytes or reusable credentials.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildResult {
    pub protocol_version: u32,
    pub request_id: Uuid,
    pub tenant_id: Uuid,
    pub attempt: u32,
    pub outcome: ModuleBuildOutcome,
    pub source_digest: String,
    pub dependency_lock_digest: String,
    pub toolchain_digest: String,
    pub wit_digest: String,
    pub component_digest: Option<String>,
    pub sbom_digest: Option<String>,
    pub provenance_digest: Option<String>,
    pub component_interface: Option<ModuleBuildComponentInterface>,
    pub evidence: ModuleBuildEvidence,
    /// Present only after the worker has published a verified successful build.
    /// Runner output must leave this unset; the owner persists a successful
    /// result only after it carries digest-pinned publication identity.
    pub publication: Option<ModuleBuildPublicationReceipt>,
    pub metrics: ModuleBuildMetrics,
    pub retryable: bool,
    pub next_action: ModuleBuildNextAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBuildOutcome {
    Succeeded,
    Failed(ModuleBuildFailureCode),
    Cancelled,
    Nondeterministic,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBuildFailureCode {
    SourceDigestMismatch,
    UnsafeArchive,
    DependencyPolicyDenied,
    BuildScriptDenied,
    NativeLinkDenied,
    ResourceLimitExceeded,
    NetworkPolicyDenied,
    ValidationFailed,
    WitContractMismatch,
    ComponentInspectionFailed,
    SbomGenerationFailed,
    ProvenanceGenerationFailed,
    PublicationFailed,
    WorkerUnavailable,
    Internal,
}

impl ModuleBuildFailureCode {
    /// Canonical lifecycle stage for a terminal worker failure.
    pub const fn diagnostic_stage(self) -> ModuleBuildDiagnosticStage {
        match self {
            Self::SourceDigestMismatch | Self::UnsafeArchive => ModuleBuildDiagnosticStage::Source,
            Self::DependencyPolicyDenied
            | Self::BuildScriptDenied
            | Self::NativeLinkDenied
            | Self::NetworkPolicyDenied => ModuleBuildDiagnosticStage::DependencyPolicy,
            Self::ValidationFailed => ModuleBuildDiagnosticStage::Validation,
            Self::WitContractMismatch | Self::ComponentInspectionFailed => {
                ModuleBuildDiagnosticStage::ComponentInspection
            }
            Self::SbomGenerationFailed | Self::ProvenanceGenerationFailed => {
                ModuleBuildDiagnosticStage::Evidence
            }
            Self::PublicationFailed => ModuleBuildDiagnosticStage::Publication,
            Self::ResourceLimitExceeded | Self::WorkerUnavailable | Self::Internal => {
                ModuleBuildDiagnosticStage::Worker
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildComponentInterface {
    pub exports: Vec<String>,
    pub imports: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildEvidence {
    pub log_reference: String,
    pub policy_report_reference: String,
    /// Ordered outcomes for requested validation profiles. Successful builds
    /// report every requested profile as passed.
    pub validation_results: Vec<ModuleBuildValidationResult>,
    /// Bounded stable diagnostics for CLI, Alloy, CI, and admin consumers.
    /// They intentionally do not contain a compiler line, file path, or runner
    /// output; consumers can retrieve the separately authorized log reference.
    #[serde(default)]
    pub diagnostics: Vec<ModuleBuildDiagnostic>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBuildDiagnosticStage {
    Source,
    DependencyPolicy,
    Build,
    Validation,
    ComponentInspection,
    Evidence,
    Publication,
    Worker,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildDiagnostic {
    pub stage: ModuleBuildDiagnosticStage,
    pub code: ModuleBuildFailureCode,
}

/// Authority that created the signature recorded by an isolated build worker.
/// Author signatures and marketplace approvals are separate governance facts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBuildSignatureAuthority {
    BuildService,
}

/// Digest-pinned OCI identities emitted only after publication of the verified
/// payload, SBOM/provenance referrers, and its Cosign signature manifest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildPublicationReceipt {
    pub artifact: OciArtifactReference,
    pub sbom_referrer: OciArtifactReference,
    pub provenance_referrer: OciArtifactReference,
    pub signature_manifest: OciArtifactReference,
    pub signature_authority: ModuleBuildSignatureAuthority,
}

impl ModuleBuildPublicationReceipt {
    fn validate(&self) -> Result<(), ModuleBuildProtocolError> {
        for reference in [
            &self.artifact,
            &self.sbom_referrer,
            &self.provenance_referrer,
            &self.signature_manifest,
        ] {
            reference
                .validate()
                .map_err(|_| ModuleBuildProtocolError::InvalidResult)?;
        }
        if self.sbom_referrer.registry != self.artifact.registry
            || self.sbom_referrer.repository != self.artifact.repository
            || self.provenance_referrer.registry != self.artifact.registry
            || self.provenance_referrer.repository != self.artifact.repository
            || self.signature_manifest.registry != self.artifact.registry
            || self.signature_manifest.repository != self.artifact.repository
        {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildMetrics {
    pub duration_ms: u64,
    pub peak_memory_bytes: u64,
    pub output_bytes: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModuleBuildNextAction {
    AdmitArtifact,
    RetryBuild,
    ReviseSource,
    EscalatePolicy,
    None,
}

/// Deployment adapter for the isolated worker protocol. Its production
/// implementation must communicate with a separately deployed worker and must
/// not run Cargo in the server or runtime process.
#[async_trait]
pub trait ModuleBuildWorker: Send + Sync {
    async fn execute_build(
        &self,
        request: ModuleBuildRequest,
    ) -> Result<ModuleBuildResult, ModuleBuildProtocolError>;
}

/// Runtime health evidence for a separately deployed module build worker.
///
/// Transport readiness must use this worker-owned probe rather than assuming
/// that a bound mTLS listener implies an available hardened build runtime.
pub trait ModuleBuildWorkerReadiness: Send + Sync {
    fn is_ready(&self) -> bool;
}

/// Durable acknowledgement of a build submission. `created` is false only
/// when the exact tenant/project idempotency key already queued this request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildSubmission {
    pub request_id: Uuid,
    pub created: bool,
}

/// Durable acknowledgement of a terminal worker result. `created` is false
/// only when the same immutable result was already recorded for this request.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildResultRecord {
    pub request_id: Uuid,
    pub created: bool,
}

/// One durable immutable build pair loaded under its tenant scope. Consumers
/// use this instead of accepting a request/result pair from a host transport.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleBuildCompletedResult {
    pub request: ModuleBuildRequest,
    pub result: ModuleBuildResult,
}

/// Owner-side durable queue for isolated module builds. It performs no source
/// materialization and never invokes the worker directly; an outbox consumer
/// owns remote delivery to the worker deployment.
#[derive(Clone)]
pub struct SeaOrmModuleBuildService {
    db: DatabaseConnection,
}

impl SeaOrmModuleBuildService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn submit(
        &self,
        request: ModuleBuildRequest,
    ) -> Result<ModuleBuildSubmission, ModuleBuildProtocolError> {
        request.validate()?;
        let tenant_id = request
            .context
            .tenant_id
            .ok_or(ModuleBuildProtocolError::InvalidRequest)?;
        if tenant_id.is_nil() {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        let request_hash = build_request_hash(&request)?;
        let request_json = serde_json::to_value(&request)
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        let transaction = self.db.begin().await.map_err(persistence_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;

        if let Some(existing) = existing_submission(
            &transaction,
            tenant_id,
            &request.project_id,
            &request.context.idempotency_key,
        )
        .await?
        {
            if existing.request_hash != request_hash {
                return Err(ModuleBuildProtocolError::IdempotencyConflict);
            }
            transaction.commit().await.map_err(persistence_error)?;
            return Ok(ModuleBuildSubmission {
                request_id: existing.request_id,
                created: false,
            });
        }

        let backend = transaction.get_database_backend();
        let inserted = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_build_requests \
                     (request_id, tenant_id, project_id, idempotency_key, request_hash, request, attempt, status, created_at) \
                     VALUES ({}, {}, {}, {}, {}, {}, {}, 'queued', {}) ON CONFLICT DO NOTHING",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(request.request_id, backend),
                    uuid_value(tenant_id, backend),
                    request.project_id.clone().into(),
                    request.context.idempotency_key.clone().into(),
                    request_hash.clone().into(),
                    sea_orm::Value::Json(Some(Box::new(request_json))),
                    i64::from(request.attempt).into(),
                ],
            ))
            .await
            .map_err(persistence_error)?;
        if inserted.rows_affected() != 1 {
            let Some(existing) = existing_submission(
                &transaction,
                tenant_id,
                &request.project_id,
                &request.context.idempotency_key,
            )
            .await?
            else {
                return Err(ModuleBuildProtocolError::Persistence(
                    "module build request identifier collision".to_string(),
                ));
            };
            if existing.request_hash != request_hash {
                return Err(ModuleBuildProtocolError::IdempotencyConflict);
            }
            transaction.commit().await.map_err(persistence_error)?;
            return Ok(ModuleBuildSubmission {
                request_id: existing.request_id,
                created: false,
            });
        }

        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    Some(tenant_id),
                    DomainEvent::ModuleBuildQueued {
                        request_id: request.request_id,
                        tenant_id,
                        project_id: request.project_id.clone(),
                        attempt: request.attempt,
                    },
                ),
            )
            .await
            .map_err(persistence_error)?;
        transaction.commit().await.map_err(persistence_error)?;
        Ok(ModuleBuildSubmission {
            request_id: request.request_id,
            created: true,
        })
    }

    /// Persists one terminal result after validating it against the immutable
    /// queued request. The worker supplies tenant correlation in its result so
    /// this host-side receiver can establish tenant RLS before loading state.
    pub async fn record_result(
        &self,
        result: ModuleBuildResult,
    ) -> Result<ModuleBuildResultRecord, ModuleBuildProtocolError> {
        if result.tenant_id.is_nil() {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        let result_hash = build_result_hash(&result)?;
        let result_json = serde_json::to_value(&result)
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        let transaction = self.db.begin().await.map_err(persistence_error)?;
        configure_tenant_scope(&transaction, result.tenant_id)
            .await
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request, status, result_hash FROM module_build_requests \
                     WHERE request_id = {}{}",
                    placeholder(backend, 1),
                    result_lock_clause(backend),
                ),
                vec![uuid_value(result.request_id, backend)],
            ))
            .await
            .map_err(persistence_error)?
            .ok_or(ModuleBuildProtocolError::UnknownRequest)?;
        let request_json: serde_json::Value =
            row.try_get("", "request").map_err(persistence_error)?;
        let request: ModuleBuildRequest = serde_json::from_value(request_json)
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        result.validate_against(&request)?;
        if matches!(&result.outcome, ModuleBuildOutcome::Succeeded) && result.publication.is_none()
        {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        let status: String = row.try_get("", "status").map_err(persistence_error)?;
        let stored_result_hash: Option<String> =
            row.try_get("", "result_hash").map_err(persistence_error)?;
        if status == "completed" {
            if stored_result_hash.as_deref() != Some(result_hash.as_str()) {
                return Err(ModuleBuildProtocolError::ResultConflict);
            }
            transaction.commit().await.map_err(persistence_error)?;
            return Ok(ModuleBuildResultRecord {
                request_id: result.request_id,
                created: false,
            });
        }
        if status != "queued" {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_build_requests \
                     SET result = {}, result_hash = {}, status = 'completed', completed_at = {} \
                     WHERE request_id = {} AND status = 'queued'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    now_expression(backend),
                    placeholder(backend, 3),
                ),
                vec![
                    sea_orm::Value::Json(Some(Box::new(result_json))),
                    result_hash.into(),
                    uuid_value(result.request_id, backend),
                ],
            ))
            .await
            .map_err(persistence_error)?;
        if updated.rows_affected() != 1 {
            return Err(ModuleBuildProtocolError::ResultConflict);
        }
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    Some(result.tenant_id),
                    DomainEvent::ModuleBuildCompleted {
                        request_id: result.request_id,
                        tenant_id: result.tenant_id,
                        outcome: build_outcome_name(&result.outcome).to_string(),
                        retryable: result.retryable,
                    },
                ),
            )
            .await
            .map_err(persistence_error)?;
        transaction.commit().await.map_err(persistence_error)?;
        Ok(ModuleBuildResultRecord {
            request_id: result.request_id,
            created: true,
        })
    }

    /// Loads a queued request under tenant RLS without retaining a database
    /// transaction during remote worker execution. An outbox consumer calls
    /// this before invoking the isolated worker and then returns its immutable
    /// result through [`Self::record_result`].
    pub async fn load_queued(
        &self,
        tenant_id: Uuid,
        request_id: Uuid,
    ) -> Result<ModuleBuildRequest, ModuleBuildProtocolError> {
        if tenant_id.is_nil() || request_id.is_nil() {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        let transaction = self.db.begin().await.map_err(persistence_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request, status FROM module_build_requests WHERE request_id = {}",
                    placeholder(backend, 1),
                ),
                vec![uuid_value(request_id, backend)],
            ))
            .await
            .map_err(persistence_error)?
            .ok_or(ModuleBuildProtocolError::UnknownRequest)?;
        let status: String = row.try_get("", "status").map_err(persistence_error)?;
        if status != "queued" {
            transaction.commit().await.map_err(persistence_error)?;
            return Err(ModuleBuildProtocolError::NotQueued);
        }
        let request_json: serde_json::Value =
            row.try_get("", "request").map_err(persistence_error)?;
        let request: ModuleBuildRequest = serde_json::from_value(request_json)
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        if request.request_id != request_id || request.context.tenant_id != Some(tenant_id) {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        request.validate()?;
        transaction.commit().await.map_err(persistence_error)?;
        Ok(request)
    }

    /// Loads one completed immutable build request/result pair under tenant RLS.
    /// The result is revalidated against its stored request before it crosses
    /// into another owner boundary such as release staging.
    pub async fn load_completed(
        &self,
        tenant_id: Uuid,
        request_id: Uuid,
    ) -> Result<ModuleBuildCompletedResult, ModuleBuildProtocolError> {
        if tenant_id.is_nil() || request_id.is_nil() {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        let transaction = self.db.begin().await.map_err(persistence_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT request, result, status FROM module_build_requests WHERE request_id = {}",
                    placeholder(backend, 1),
                ),
                vec![uuid_value(request_id, backend)],
            ))
            .await
            .map_err(persistence_error)?
            .ok_or(ModuleBuildProtocolError::UnknownRequest)?;
        let status: String = row.try_get("", "status").map_err(persistence_error)?;
        if status != "completed" {
            transaction.commit().await.map_err(persistence_error)?;
            return Err(ModuleBuildProtocolError::NotCompleted);
        }
        let request_json: serde_json::Value =
            row.try_get("", "request").map_err(persistence_error)?;
        let result_json: Option<serde_json::Value> =
            row.try_get("", "result").map_err(persistence_error)?;
        let request: ModuleBuildRequest = serde_json::from_value(request_json)
            .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        let result: ModuleBuildResult =
            serde_json::from_value(result_json.ok_or(ModuleBuildProtocolError::InvalidResult)?)
                .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
        result.validate_against(&request)?;
        if request.context.tenant_id != Some(tenant_id) || result.tenant_id != tenant_id {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        transaction.commit().await.map_err(persistence_error)?;
        Ok(ModuleBuildCompletedResult { request, result })
    }

    /// Delivers one outbox-selected request to the isolated worker. The owner
    /// never executes Cargo itself: it loads the immutable queued request,
    /// releases the database transaction, delegates through the worker port,
    /// and validates/persists the returned terminal result.
    pub async fn dispatch_queued<W>(
        &self,
        tenant_id: Uuid,
        request_id: Uuid,
        worker: &W,
    ) -> Result<ModuleBuildResultRecord, ModuleBuildProtocolError>
    where
        W: ModuleBuildWorker + ?Sized,
    {
        let request = self.load_queued(tenant_id, request_id).await?;
        let result = worker.execute_build(request).await?;
        self.record_result(result).await
    }
}

impl ModuleBuildRequest {
    pub fn validate(&self) -> Result<(), ModuleBuildProtocolError> {
        if self.protocol_version != MODULE_BUILD_PROTOCOL_VERSION
            || self.request_id.is_nil()
            || self.project_id.trim().is_empty()
            || self.attempt == 0
        {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        self.context
            .validate()
            .map_err(|_| ModuleBuildProtocolError::InvalidRequest)?;
        if !matches!(self.context.tenant_id, Some(tenant_id) if !tenant_id.is_nil()) {
            return Err(ModuleBuildProtocolError::InvalidRequest);
        }
        for value in [
            &self.context.actor_id,
            &self.context.trace_id,
            &self.context.correlation_id,
            &self.context.idempotency_key,
        ] {
            validate_text(value)?;
        }
        validate_text(&self.project_id)?;
        validate_module_slug(&self.expected_module_slug)?;
        validate_text(&self.expected_version)?;
        validate_text(&self.runtime_abi)?;
        validate_text(&self.wit.world)?;
        validate_text(&self.wit.version)?;
        validate_text(&self.toolchain.rust_toolchain)?;
        validate_text(&self.toolchain.component_target)?;
        self.authoring.validate()?;
        if !valid_digest(&self.source.digest) || !valid_digest(&self.dependency_policy.lock_digest)
        {
            return Err(ModuleBuildProtocolError::InvalidDigest);
        }
        validate_build_source(&self.source)?;
        validate_dependency_policy(&self.dependency_policy)?;
        self.limits.validate()?;
        validate_network_policy(&self.network_policy)?;
        if self.validation_profiles.is_empty()
            || self.validation_profiles.len() > MAX_VALIDATION_PROFILES
            || self
                .validation_profiles
                .iter()
                .enumerate()
                .any(|(index, profile)| self.validation_profiles[..index].contains(profile))
        {
            return Err(ModuleBuildProtocolError::InvalidValidationProfiles);
        }
        Ok(())
    }
}

impl ModuleBuildLimits {
    pub fn validate(&self) -> Result<(), ModuleBuildProtocolError> {
        if self.cpu_cores == 0
            || self.cpu_cores > MAX_CPU_CORES
            || self.memory_bytes == 0
            || self.memory_bytes > MAX_MEMORY_BYTES
            || self.disk_bytes == 0
            || self.disk_bytes > MAX_DISK_BYTES
            || self.process_limit == 0
            || self.process_limit > MAX_PROCESSES
            || self.output_bytes == 0
            || self.output_bytes > MAX_OUTPUT_BYTES
            || self.wall_clock_ms == 0
            || self.wall_clock_ms > MAX_WALL_CLOCK_MS
        {
            return Err(ModuleBuildProtocolError::InvalidLimits);
        }
        Ok(())
    }
}

impl ModuleBuildResult {
    pub fn validate_against(
        &self,
        request: &ModuleBuildRequest,
    ) -> Result<(), ModuleBuildProtocolError> {
        request.validate()?;
        let expected_toolchain_digest = request.toolchain.protocol_digest();
        let expected_wit_digest = request.wit.protocol_digest();
        if self.protocol_version != MODULE_BUILD_PROTOCOL_VERSION
            || self.protocol_version != request.protocol_version
            || self.request_id != request.request_id
            || Some(self.tenant_id) != request.context.tenant_id
            || self.attempt != request.attempt
            || self.source_digest != request.source.digest
            || self.dependency_lock_digest != request.dependency_policy.lock_digest
            || self.toolchain_digest != expected_toolchain_digest
            || self.wit_digest != expected_wit_digest
            || self.metrics.duration_ms > request.limits.wall_clock_ms
            || self.metrics.peak_memory_bytes > request.limits.memory_bytes
            || self.metrics.output_bytes > request.limits.output_bytes
        {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        for digest in [
            &self.component_digest,
            &self.sbom_digest,
            &self.provenance_digest,
        ]
        .into_iter()
        .flatten()
        {
            if !valid_digest(digest) {
                return Err(ModuleBuildProtocolError::InvalidResult);
            }
        }
        if self
            .component_interface
            .as_ref()
            .is_some_and(|interface| !valid_component_interface(interface))
        {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        if let Some(publication) = &self.publication {
            publication.validate()?;
        }
        if self.retryable != (self.next_action == ModuleBuildNextAction::RetryBuild) {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        if !valid_validation_results(
            &self.evidence.validation_results,
            &request.validation_profiles,
            &self.outcome,
        ) {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        match &self.outcome {
            ModuleBuildOutcome::Succeeded => {
                if self.component_digest.is_none()
                    || self.sbom_digest.is_none()
                    || self.provenance_digest.is_none()
                    || self.component_interface.is_none()
                    || self.retryable
                    || self.next_action != ModuleBuildNextAction::AdmitArtifact
                {
                    return Err(ModuleBuildProtocolError::InvalidResult);
                }
            }
            ModuleBuildOutcome::Failed(_) | ModuleBuildOutcome::Cancelled => {
                if self.next_action == ModuleBuildNextAction::AdmitArtifact
                    || self.publication.is_some()
                    || self.component_digest.is_some()
                    || self.sbom_digest.is_some()
                    || self.provenance_digest.is_some()
                    || self.component_interface.is_some()
                {
                    return Err(ModuleBuildProtocolError::InvalidResult);
                }
            }
            ModuleBuildOutcome::Nondeterministic => {
                if self.retryable
                    || self.next_action != ModuleBuildNextAction::EscalatePolicy
                    || self.publication.is_some()
                {
                    return Err(ModuleBuildProtocolError::InvalidResult);
                }
            }
        }
        validate_reference(&self.evidence.log_reference)?;
        validate_reference(&self.evidence.policy_report_reference)?;
        if self.evidence.diagnostics.len() > MAX_BUILD_DIAGNOSTICS
            || self
                .evidence
                .diagnostics
                .iter()
                .enumerate()
                .any(|(index, diagnostic)| {
                    self.evidence.diagnostics[..index]
                        .iter()
                        .any(|previous| previous == diagnostic)
                })
            || self
                .evidence
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.stage != diagnostic.code.diagnostic_stage())
        {
            return Err(ModuleBuildProtocolError::InvalidResult);
        }
        match &self.outcome {
            ModuleBuildOutcome::Failed(failure)
                if !self
                    .evidence
                    .diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.code == *failure) =>
            {
                return Err(ModuleBuildProtocolError::InvalidResult);
            }
            ModuleBuildOutcome::Succeeded if !self.evidence.diagnostics.is_empty() => {
                return Err(ModuleBuildProtocolError::InvalidResult);
            }
            _ => {}
        }
        Ok(())
    }
}

fn validate_dependency_policy(
    policy: &ModuleBuildDependencyPolicy,
) -> Result<(), ModuleBuildProtocolError> {
    if policy.allowed_registries.len() > MAX_ALLOWED_REGISTRIES
        || policy
            .allowed_registries
            .iter()
            .enumerate()
            .any(|(index, registry)| {
                registry.trim().is_empty()
                    || registry.len() > MAX_BUILD_REFERENCE_BYTES
                    || policy.allowed_registries[..index].contains(registry)
            })
    {
        return Err(ModuleBuildProtocolError::InvalidDependencyPolicy);
    }
    Ok(())
}

fn validate_network_policy(
    policy: &ModuleBuildNetworkPolicy,
) -> Result<(), ModuleBuildProtocolError> {
    match policy {
        ModuleBuildNetworkPolicy::Denied => Ok(()),
        ModuleBuildNetworkPolicy::ScopedDependencyMaterialization { endpoints }
            if !endpoints.is_empty()
                && endpoints.len() <= MAX_SCOPED_ENDPOINTS
                && endpoints.iter().enumerate().all(|(index, endpoint)| {
                    endpoint.starts_with("https://")
                        && endpoint.len() <= MAX_BUILD_REFERENCE_BYTES
                        && !endpoints[..index].contains(endpoint)
                }) =>
        {
            Ok(())
        }
        ModuleBuildNetworkPolicy::ScopedDependencyMaterialization { .. } => {
            Err(ModuleBuildProtocolError::InvalidNetworkPolicy)
        }
    }
}

fn validate_reference(value: &str) -> Result<(), ModuleBuildProtocolError> {
    if value.trim().is_empty()
        || value.len() > MAX_BUILD_REFERENCE_BYTES
        || value.contains(char::is_whitespace)
    {
        return Err(ModuleBuildProtocolError::InvalidReference);
    }
    Ok(())
}

fn validate_build_source(source: &ModuleBuildSource) -> Result<(), ModuleBuildProtocolError> {
    if !valid_digest(&source.digest) || source.reference != format!("cas://{}", source.digest) {
        return Err(ModuleBuildProtocolError::InvalidReference);
    }
    Ok(())
}

fn valid_component_interface(interface: &ModuleBuildComponentInterface) -> bool {
    for names in [&interface.exports, &interface.imports] {
        if names.len() > 128
            || names.iter().enumerate().any(|(index, name)| {
                name.trim().is_empty()
                    || name.len() > MAX_BUILD_TEXT_BYTES
                    || name.contains(char::is_control)
                    || names[..index].contains(name)
            })
        {
            return false;
        }
    }
    true
}

fn valid_validation_results(
    results: &[ModuleBuildValidationResult],
    requested: &[ModuleBuildValidationProfile],
    outcome: &ModuleBuildOutcome,
) -> bool {
    if results.len() > requested.len()
        || results
            .iter()
            .enumerate()
            .any(|(index, result)| requested.get(index) != Some(&result.profile))
    {
        return false;
    }
    match outcome {
        ModuleBuildOutcome::Succeeded => {
            results.len() == requested.len()
                && results
                    .iter()
                    .all(|result| result.outcome == ModuleBuildValidationOutcome::Passed)
        }
        ModuleBuildOutcome::Failed(ModuleBuildFailureCode::ValidationFailed) => {
            !results.is_empty()
                && results
                    .last()
                    .is_some_and(|result| result.outcome == ModuleBuildValidationOutcome::Failed)
                && results[..results.len() - 1]
                    .iter()
                    .all(|result| result.outcome == ModuleBuildValidationOutcome::Passed)
        }
        ModuleBuildOutcome::Failed(_)
        | ModuleBuildOutcome::Cancelled
        | ModuleBuildOutcome::Nondeterministic => results.is_empty(),
    }
}

fn validate_text(value: &str) -> Result<(), ModuleBuildProtocolError> {
    if value.trim().is_empty()
        || value.len() > MAX_BUILD_TEXT_BYTES
        || value.contains(char::is_control)
    {
        return Err(ModuleBuildProtocolError::InvalidRequest);
    }
    Ok(())
}

fn validate_module_slug(value: &str) -> Result<(), ModuleBuildProtocolError> {
    if value.is_empty()
        || value.len() > 48
        || value.starts_with('_')
        || value.ends_with('_')
        || value.chars().any(|character| {
            !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '_'
        })
    {
        return Err(ModuleBuildProtocolError::InvalidRequest);
    }
    Ok(())
}

fn valid_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

/// Produces a domain-separated SHA-256 digest over bounded protocol strings.
/// Request validation rejects control characters, so NUL safely delimits the
/// v1 fields without relying on a serializer implementation for evidence
/// identity.
fn protocol_digest(domain: &str, fields: &[&str]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(domain.as_bytes());
    for field in fields {
        hasher.update([0]);
        hasher.update(field.as_bytes());
    }
    format!("sha256:{}", hex::encode(hasher.finalize()))
}

struct ExistingSubmission {
    request_id: Uuid,
    request_hash: String,
}

async fn existing_submission<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    project_id: &str,
    idempotency_key: &str,
) -> Result<Option<ExistingSubmission>, ModuleBuildProtocolError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT request_id, request_hash FROM module_build_requests \
                 WHERE tenant_id = {} AND project_id = {} AND idempotency_key = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(tenant_id, backend),
                project_id.to_owned().into(),
                idempotency_key.to_owned().into(),
            ],
        ))
        .await
        .map_err(persistence_error)?;
    row.map(|row| {
        Ok(ExistingSubmission {
            request_id: uuid_from_row(&row, "request_id", backend)
                .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?,
            request_hash: row.try_get("", "request_hash").map_err(persistence_error)?,
        })
    })
    .transpose()
}

fn build_request_hash(request: &ModuleBuildRequest) -> Result<String, ModuleBuildProtocolError> {
    let serialized = serde_json::to_vec(request)
        .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(serialized))
    ))
}

fn build_result_hash(result: &ModuleBuildResult) -> Result<String, ModuleBuildProtocolError> {
    let serialized = serde_json::to_vec(result)
        .map_err(|error| ModuleBuildProtocolError::Persistence(error.to_string()))?;
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(serialized))
    ))
}

fn build_outcome_name(outcome: &ModuleBuildOutcome) -> &'static str {
    match outcome {
        ModuleBuildOutcome::Succeeded => "succeeded",
        ModuleBuildOutcome::Failed(_) => "failed",
        ModuleBuildOutcome::Cancelled => "cancelled",
        ModuleBuildOutcome::Nondeterministic => "nondeterministic",
    }
}

fn result_lock_clause(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Postgres => " FOR UPDATE",
        _ => "",
    }
}

fn persistence_error(error: impl std::fmt::Display) -> ModuleBuildProtocolError {
    ModuleBuildProtocolError::Persistence(error.to_string())
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ModuleBuildProtocolError {
    #[error("module build request is invalid")]
    InvalidRequest,
    #[error("module build digest is invalid")]
    InvalidDigest,
    #[error("module build reference is invalid")]
    InvalidReference,
    #[error("module build dependency policy is invalid")]
    InvalidDependencyPolicy,
    #[error("module build resource limits are invalid")]
    InvalidLimits,
    #[error("module build network policy is invalid")]
    InvalidNetworkPolicy,
    #[error("module build validation profiles are invalid")]
    InvalidValidationProfiles,
    #[error("module build worker result is inconsistent with its request")]
    InvalidResult,
    #[error("module build idempotency key was reused for a different request")]
    IdempotencyConflict,
    #[error("module build request does not exist in the tenant scope")]
    UnknownRequest,
    #[error("module build request is no longer queued")]
    NotQueued,
    #[error("module build request is not completed")]
    NotCompleted,
    #[error("module build request already has a different terminal result")]
    ResultConflict,
    #[error("module build transport failed: {0}")]
    Transport(String),
    #[error("module build persistence failed: {0}")]
    Persistence(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(marker: char) -> String {
        format!("sha256:{}", marker.to_string().repeat(64))
    }

    fn request() -> ModuleBuildRequest {
        ModuleBuildRequest {
            protocol_version: MODULE_BUILD_PROTOCOL_VERSION,
            request_id: Uuid::new_v4(),
            context: ModuleCommandContext {
                actor_id: "user:42".to_string(),
                tenant_id: Some(Uuid::new_v4()),
                trace_id: "trace-1".to_string(),
                correlation_id: "correlation-1".to_string(),
                idempotency_key: "module-build:1".to_string(),
            },
            project_id: "project:sample".to_string(),
            source: ModuleBuildSource {
                digest: digest('a'),
                reference: format!("cas://{}", digest('a')),
            },
            expected_module_slug: "sample_module".to_string(),
            expected_version: "1.0.0".to_string(),
            runtime_abi: "rustok:module/runtime@1".to_string(),
            wit: ModuleBuildWitContract {
                world: "rustok:module/module".to_string(),
                version: "1.0.0".to_string(),
            },
            toolchain: ModuleBuildToolchain {
                rust_toolchain: "1.85.0".to_string(),
                component_target: "wasm32-wasip2".to_string(),
            },
            authoring: ModuleBuildAuthoring {
                sdk_version: "1.0.0".to_string(),
                template_version: "1.0.0".to_string(),
            },
            dependency_policy: ModuleBuildDependencyPolicy {
                lock_digest: digest('b'),
                allowed_registries: vec!["https://crates.io".to_string()],
                allow_git_dependencies: false,
                allow_build_scripts: false,
                allow_native_links: false,
            },
            limits: ModuleBuildLimits {
                cpu_cores: 2,
                memory_bytes: 512 * 1024 * 1024,
                disk_bytes: 2 * 1024 * 1024 * 1024,
                process_limit: 32,
                output_bytes: 1024 * 1024,
                wall_clock_ms: 300_000,
            },
            network_policy: ModuleBuildNetworkPolicy::Denied,
            validation_profiles: vec![
                ModuleBuildValidationProfile::Check,
                ModuleBuildValidationProfile::Test,
            ],
            attempt: 1,
        }
    }

    #[test]
    fn request_rejects_unbounded_network_and_duplicate_profiles() {
        let mut request = request();
        request.network_policy = ModuleBuildNetworkPolicy::ScopedDependencyMaterialization {
            endpoints: Vec::new(),
        };
        assert!(matches!(
            request.validate(),
            Err(ModuleBuildProtocolError::InvalidNetworkPolicy)
        ));

        request.network_policy = ModuleBuildNetworkPolicy::Denied;
        request.validation_profiles = vec![
            ModuleBuildValidationProfile::Check,
            ModuleBuildValidationProfile::Check,
        ];
        assert!(matches!(
            request.validate(),
            Err(ModuleBuildProtocolError::InvalidValidationProfiles)
        ));

        request.validation_profiles = vec![ModuleBuildValidationProfile::Check];
        request.authoring.sdk_version = "unversioned".to_string();
        assert!(matches!(
            request.validate(),
            Err(ModuleBuildProtocolError::InvalidRequest)
        ));
    }

    #[test]
    fn successful_result_requires_admission_evidence() {
        let request = request();
        let result = ModuleBuildResult {
            protocol_version: MODULE_BUILD_PROTOCOL_VERSION,
            request_id: request.request_id,
            tenant_id: request.context.tenant_id.expect("tenant"),
            attempt: request.attempt,
            outcome: ModuleBuildOutcome::Succeeded,
            source_digest: request.source.digest.clone(),
            dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
            toolchain_digest: request.toolchain.protocol_digest(),
            wit_digest: request.wit.protocol_digest(),
            component_digest: None,
            sbom_digest: None,
            provenance_digest: None,
            component_interface: None,
            evidence: ModuleBuildEvidence {
                log_reference: "cas://logs/1".to_string(),
                policy_report_reference: "cas://reports/1".to_string(),
                validation_results: Vec::new(),
                diagnostics: Vec::new(),
            },
            publication: None,
            metrics: ModuleBuildMetrics {
                duration_ms: 1,
                peak_memory_bytes: 1,
                output_bytes: 1,
            },
            retryable: false,
            next_action: ModuleBuildNextAction::AdmitArtifact,
        };
        assert!(matches!(
            result.validate_against(&request),
            Err(ModuleBuildProtocolError::InvalidResult)
        ));
    }

    #[test]
    fn publication_receipt_requires_a_signature_in_the_artifact_repository() {
        let reference = |marker| OciArtifactReference {
            registry: "registry.example".to_string(),
            repository: "modules/sample_module".to_string(),
            digest: digest(marker),
        };
        let mut receipt = ModuleBuildPublicationReceipt {
            artifact: reference('a'),
            sbom_referrer: reference('b'),
            provenance_referrer: reference('c'),
            signature_manifest: reference('d'),
            signature_authority: ModuleBuildSignatureAuthority::BuildService,
        };
        assert!(receipt.validate().is_ok());
        assert_eq!(
            receipt.signature_authority,
            ModuleBuildSignatureAuthority::BuildService
        );

        receipt.signature_manifest.repository = "modules/other_module".to_string();
        assert!(matches!(
            receipt.validate(),
            Err(ModuleBuildProtocolError::InvalidResult)
        ));
    }

    #[test]
    fn failed_result_requires_a_matching_structured_diagnostic() {
        let request = request();
        let mut result = ModuleBuildResult {
            protocol_version: MODULE_BUILD_PROTOCOL_VERSION,
            request_id: request.request_id,
            tenant_id: request.context.tenant_id.expect("tenant"),
            attempt: request.attempt,
            outcome: ModuleBuildOutcome::Failed(ModuleBuildFailureCode::ValidationFailed),
            source_digest: request.source.digest.clone(),
            dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
            toolchain_digest: request.toolchain.protocol_digest(),
            wit_digest: request.wit.protocol_digest(),
            component_digest: None,
            sbom_digest: None,
            provenance_digest: None,
            component_interface: None,
            evidence: ModuleBuildEvidence {
                log_reference: "cas://logs/1".to_string(),
                policy_report_reference: "cas://reports/1".to_string(),
                validation_results: Vec::new(),
                diagnostics: Vec::new(),
            },
            publication: None,
            metrics: ModuleBuildMetrics {
                duration_ms: 1,
                peak_memory_bytes: 1,
                output_bytes: 1,
            },
            retryable: false,
            next_action: ModuleBuildNextAction::ReviseSource,
        };
        assert!(matches!(
            result.validate_against(&request),
            Err(ModuleBuildProtocolError::InvalidResult)
        ));

        result.evidence.diagnostics = vec![ModuleBuildDiagnostic {
            stage: ModuleBuildDiagnosticStage::Validation,
            code: ModuleBuildFailureCode::ValidationFailed,
        }];
        result.evidence.validation_results = vec![ModuleBuildValidationResult {
            profile: ModuleBuildValidationProfile::Check,
            outcome: ModuleBuildValidationOutcome::Failed,
        }];
        let validation = result.validate_against(&request);
        assert!(validation.is_ok(), "{validation:?}");

        result.evidence.diagnostics[0].stage = ModuleBuildDiagnosticStage::Build;
        assert!(matches!(
            result.validate_against(&request),
            Err(ModuleBuildProtocolError::InvalidResult)
        ));
    }

    #[test]
    fn result_rejects_a_toolchain_or_wit_digest_not_bound_to_the_request() {
        let request = request();
        let mut result = ModuleBuildResult {
            protocol_version: MODULE_BUILD_PROTOCOL_VERSION,
            request_id: request.request_id,
            tenant_id: request.context.tenant_id.expect("tenant"),
            attempt: request.attempt,
            outcome: ModuleBuildOutcome::Succeeded,
            source_digest: request.source.digest.clone(),
            dependency_lock_digest: request.dependency_policy.lock_digest.clone(),
            toolchain_digest: request.toolchain.protocol_digest(),
            wit_digest: request.wit.protocol_digest(),
            component_digest: Some(digest('c')),
            sbom_digest: Some(digest('d')),
            provenance_digest: Some(digest('e')),
            component_interface: Some(ModuleBuildComponentInterface {
                exports: vec!["run".to_string()],
                imports: Vec::new(),
            }),
            evidence: ModuleBuildEvidence {
                log_reference: "cas://logs/1".to_string(),
                policy_report_reference: "cas://reports/1".to_string(),
                validation_results: vec![
                    ModuleBuildValidationResult {
                        profile: ModuleBuildValidationProfile::Check,
                        outcome: ModuleBuildValidationOutcome::Passed,
                    },
                    ModuleBuildValidationResult {
                        profile: ModuleBuildValidationProfile::Test,
                        outcome: ModuleBuildValidationOutcome::Passed,
                    },
                ],
                diagnostics: Vec::new(),
            },
            publication: None,
            metrics: ModuleBuildMetrics {
                duration_ms: 1,
                peak_memory_bytes: 1,
                output_bytes: 1,
            },
            retryable: false,
            next_action: ModuleBuildNextAction::AdmitArtifact,
        };
        let validation = result.validate_against(&request);
        assert!(validation.is_ok(), "{validation:?}");

        result.wit_digest = digest('f');
        assert!(matches!(
            result.validate_against(&request),
            Err(ModuleBuildProtocolError::InvalidResult)
        ));

        result.wit_digest = request.wit.protocol_digest();
        result.outcome = ModuleBuildOutcome::Failed(ModuleBuildFailureCode::WorkerUnavailable);
        result.evidence.validation_results.clear();
        result.evidence.diagnostics = vec![ModuleBuildDiagnostic {
            stage: ModuleBuildDiagnosticStage::Worker,
            code: ModuleBuildFailureCode::WorkerUnavailable,
        }];
        result.component_digest = None;
        result.sbom_digest = None;
        result.provenance_digest = None;
        result.component_interface = None;
        result.retryable = true;
        result.next_action = ModuleBuildNextAction::ReviseSource;
        assert!(matches!(
            result.validate_against(&request),
            Err(ModuleBuildProtocolError::InvalidResult)
        ));

        result.next_action = ModuleBuildNextAction::RetryBuild;
        assert!(result.validate_against(&request).is_ok());
    }
}
