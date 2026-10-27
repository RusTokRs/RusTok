//! Verified payload caching, authenticated prefetch/readiness across executor pools/generations,
//! owner-selected executor placement enforcement, and dual candidate/predecessor smoke readiness.
//!
//! Enforces:
//! - Stable dynamic `RuntimeFingerprint` (engine/binary/config/target/ABI).
//! - Monotonic `pool_generation` gating: generation change invalidates smoke readiness.
//! - Capability-route parity: every declared binding capability must have an active broker route.
//! - Owner-selected placement: author-declared placement is ignored; required isolation has no in-process fallback.
//! - Dual candidate & predecessor smoke readiness for automatic mode.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, Statement};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use rustok_sandbox::SandboxExecutorPlacement;

use crate::backend::SupportedBackend;
use crate::installation::{sha256_digest, valid_digest};
use crate::{
    ArtifactBlobStore, ArtifactPayloadKind, ControlPlaneInfrastructure, InstalledModuleArtifact,
    ModuleArtifactDescriptor, ModuleCommandContext, ModuleInstallationError, ModuleRuntimeBinding,
    ModuleRuntimeBindingKind,
};

#[derive(Debug, thiserror::Error)]
pub enum ExecutorReadinessError {
    #[error("Invalid runtime fingerprint: {0}")]
    InvalidFingerprint(String),
    #[error("Invalid pool identity: {0}")]
    InvalidPoolIdentity(String),
    #[error(
        "Isolated worker execution is required by owner policy, but pool placement is in-process (in-process fallback is strictly prohibited)"
    )]
    IsolatedWorkerRequired,
    #[error("Missing or invalid placement attestation for isolated worker pool")]
    MissingWorkerAttestation,
    #[error("Missing declared capability route: {0}")]
    MissingCapabilityRoute(String),
    #[error(
        "Engine or configuration changed: previous readiness receipt invalidated (receipt fingerprint `{0}` != active fingerprint `{1}`)"
    )]
    EngineFingerprintMismatch(String, String),
    #[error(
        "Stale pool generation: receipt generation {0} does not match current pool generation {1} (smoke readiness must repeat on generation change)"
    )]
    StalePoolGeneration(u64, u64),
    #[error("Smoke binding execution failed: {0}")]
    SmokeExecutionFailed(String),
    #[error("Executor readiness command is invalid: {0}")]
    InvalidCommand(String),
    #[error("Executor readiness authorization denied")]
    AuthorizationDenied,
    #[error("Executor readiness probe is unavailable: {0}")]
    ProbeUnavailable(String),
    #[error("Executor pool does not support payload kind `{0:?}`")]
    UnsupportedPayloadKind(ArtifactPayloadKind),
    #[error("Executor readiness configuration is invalid: {0}")]
    InvalidConfiguration(String),
    #[error("Automatic mode denied: {0}")]
    AutomaticModeDenied(String),
    #[error("Blob store error: {0}")]
    BlobStore(#[from] ModuleInstallationError),
    #[error("Store error: {0}")]
    Store(String),
}

/// Dynamic runtime fingerprint: the stable digest of the exact executor binary,
/// engine build, engine configuration revision, isolated-worker image digest,
/// target CPU contract, and runtime ABI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFingerprint {
    pub executor_binary_digest: String,
    pub engine_build_digest: String,
    pub engine_config_revision: String,
    pub isolated_worker_image_digest: Option<String>,
    pub target_cpu_contract: String,
    pub runtime_abi: String,
}

impl RuntimeFingerprint {
    pub fn compute_digest(&self) -> String {
        let mut hasher = Sha256::new();
        hasher.update(b"runtime_fingerprint:v1\n");
        hasher.update(format!("executor_binary:{}\n", self.executor_binary_digest).as_bytes());
        hasher.update(format!("engine_build:{}\n", self.engine_build_digest).as_bytes());
        hasher.update(format!("engine_config:{}\n", self.engine_config_revision).as_bytes());
        hasher.update(
            format!(
                "isolated_worker_image:{}\n",
                self.isolated_worker_image_digest
                    .as_deref()
                    .unwrap_or("none")
            )
            .as_bytes(),
        );
        hasher.update(format!("target_cpu:{}\n", self.target_cpu_contract).as_bytes());
        hasher.update(format!("runtime_abi:{}\n", self.runtime_abi).as_bytes());
        format!("sha256:{}", hex::encode(hasher.finalize()))
    }

    pub fn validate(&self) -> Result<(), ExecutorReadinessError> {
        if !valid_digest(&self.executor_binary_digest) {
            return Err(ExecutorReadinessError::InvalidFingerprint(
                "executor_binary_digest must be a valid sha256 digest".to_string(),
            ));
        }
        if !valid_digest(&self.engine_build_digest) {
            return Err(ExecutorReadinessError::InvalidFingerprint(
                "engine_build_digest must be a valid sha256 digest".to_string(),
            ));
        }
        if self.engine_config_revision.trim().is_empty() {
            return Err(ExecutorReadinessError::InvalidFingerprint(
                "engine_config_revision must not be empty".to_string(),
            ));
        }
        if let Some(ref image) = self.isolated_worker_image_digest
            && !valid_digest(image)
        {
            return Err(ExecutorReadinessError::InvalidFingerprint(
                "isolated_worker_image_digest must be a valid sha256 digest".to_string(),
            ));
        }
        if self.target_cpu_contract.trim().is_empty() {
            return Err(ExecutorReadinessError::InvalidFingerprint(
                "target_cpu_contract must not be empty".to_string(),
            ));
        }
        if self.runtime_abi.trim().is_empty() {
            return Err(ExecutorReadinessError::InvalidFingerprint(
                "runtime_abi must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

/// Identity of an executor pool with monotonic pool generation and placement.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutorPoolIdentity {
    pub pool_id: String,
    pub pool_generation: u64,
    pub fingerprint: RuntimeFingerprint,
    pub placement: SandboxExecutorPlacement,
    pub placement_attestation: Option<String>,
}

impl ExecutorPoolIdentity {
    pub fn validate(&self) -> Result<(), ExecutorReadinessError> {
        if self.pool_id.trim().is_empty() || self.pool_id.len() > 128 {
            return Err(ExecutorReadinessError::InvalidPoolIdentity(
                "pool_id must be non-empty and <= 128 characters".to_string(),
            ));
        }
        if self.pool_generation == 0 {
            return Err(ExecutorReadinessError::InvalidPoolIdentity(
                "pool_generation must be > 0".to_string(),
            ));
        }
        self.fingerprint.validate()?;
        if self.placement == SandboxExecutorPlacement::IsolatedWorker {
            if self.fingerprint.isolated_worker_image_digest.is_none() {
                return Err(ExecutorReadinessError::InvalidPoolIdentity(
                    "isolated_worker placement requires isolated_worker_image_digest in fingerprint"
                        .to_string(),
                ));
            }
            if self
                .placement_attestation
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(ExecutorReadinessError::MissingWorkerAttestation);
            }
        }
        Ok(())
    }
}

/// Canonical owner-selected executor placement policy.
///
/// Authors may declare executor kind/ABI in descriptors, but cannot select trust
/// placement. Author-declared placement is strictly ignored/rejected.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OwnerPlacementPolicy {
    pub policy_revision: u64,
    pub required_placement: SandboxExecutorPlacement,
    pub allow_in_process_fallback: bool,
}

impl OwnerPlacementPolicy {
    pub fn enforce(&self, pool: &ExecutorPoolIdentity) -> Result<(), ExecutorReadinessError> {
        if self.policy_revision == 0 {
            return Err(ExecutorReadinessError::InvalidConfiguration(
                "placement policy revision must be positive".to_string(),
            ));
        }
        if self.required_placement == SandboxExecutorPlacement::IsolatedWorker {
            if pool.placement != SandboxExecutorPlacement::IsolatedWorker {
                return Err(ExecutorReadinessError::IsolatedWorkerRequired);
            }
            if pool
                .placement_attestation
                .as_ref()
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(ExecutorReadinessError::MissingWorkerAttestation);
            }
        }
        Ok(())
    }
}

/// In-memory cache entry for verified, prepared payloads.
#[derive(Debug, Clone)]
pub struct CachedPreparedPayload {
    pub payload_digest: String,
    pub runtime_fingerprint: String,
    pub payload_bytes: Vec<u8>,
    pub verified_at: DateTime<Utc>,
}

/// Default entry bound for [`VerifiedPayloadCache`].
pub const DEFAULT_VERIFIED_PAYLOAD_CACHE_MAX_ENTRIES: usize = 128;
/// Default aggregate payload-bytes bound for [`VerifiedPayloadCache`].
pub const DEFAULT_VERIFIED_PAYLOAD_CACHE_MAX_BYTES: usize = 256 * 1024 * 1024;

/// Thread-safe, bounded verified payload cache keyed by
/// `(payload_digest, runtime_fingerprint)`.
///
/// The cache is an optimization over re-reading verified CAS payloads, never
/// an authority: a poisoned lock degrades to cache misses/no-ops instead of
/// panicking, and insertion evicts the oldest-verified entries until both
/// the entry count and aggregate byte bounds hold. An entry larger than the
/// byte bound is not retained.
#[derive(Debug)]
pub struct VerifiedPayloadCache {
    entries: RwLock<HashMap<(String, String), CachedPreparedPayload>>,
    max_entries: usize,
    max_bytes: usize,
}

impl Default for VerifiedPayloadCache {
    fn default() -> Self {
        Self::new()
    }
}

impl VerifiedPayloadCache {
    pub fn new() -> Self {
        Self::with_limits(
            DEFAULT_VERIFIED_PAYLOAD_CACHE_MAX_ENTRIES,
            DEFAULT_VERIFIED_PAYLOAD_CACHE_MAX_BYTES,
        )
    }

    pub fn with_limits(max_entries: usize, max_bytes: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            max_entries,
            max_bytes,
        }
    }

    pub fn get(
        &self,
        payload_digest: &str,
        fingerprint_digest: &str,
    ) -> Option<CachedPreparedPayload> {
        self.entries
            .read()
            .ok()?
            .get(&(payload_digest.to_string(), fingerprint_digest.to_string()))
            .cloned()
    }

    pub fn put(&self, entry: CachedPreparedPayload) {
        if entry.payload_bytes.len() > self.max_bytes || self.max_entries == 0 {
            return;
        }
        let key = (
            entry.payload_digest.clone(),
            entry.runtime_fingerprint.clone(),
        );
        let Ok(mut entries) = self.entries.write() else {
            return;
        };
        entries.insert(key, entry);
        // Evict oldest-verified entries until both bounds hold.
        while entries.len() > self.max_entries
            || entries
                .values()
                .map(|cached| cached.payload_bytes.len())
                .sum::<usize>()
                > self.max_bytes
        {
            let Some(oldest_key) = entries
                .iter()
                .min_by_key(|(_, cached)| cached.verified_at)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            entries.remove(&oldest_key);
        }
    }

    pub fn invalidate_fingerprint(&self, fingerprint_digest: &str) {
        if let Ok(mut entries) = self.entries.write() {
            entries.retain(|(_, fp), _| fp != fingerprint_digest);
        }
    }

    pub fn clear(&self) {
        if let Ok(mut entries) = self.entries.write() {
            entries.clear();
        }
    }

    pub fn len(&self) -> usize {
        self.entries
            .read()
            .map(|entries| entries.len())
            .unwrap_or(0)
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Default validity window for a smoke receipt. Readiness is evidence about
/// a live executor pool, not a permanent property of immutable bytes.
pub const DEFAULT_EXECUTOR_READINESS_RECEIPT_TTL_SECONDS: u32 = 15 * 60;
/// Maximum validity window accepted by [`ExecutorReadinessService`].
pub const MAX_EXECUTOR_READINESS_RECEIPT_TTL_SECONDS: u32 = 24 * 60 * 60;

/// Durable receipt recording one real, successful smoke execution on a
/// specific executor fingerprint, pool generation, placement, and placement
/// policy revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutorSmokeReceipt {
    pub id: Uuid,
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub release_digest: String,
    pub payload_digest: String,
    pub runtime_fingerprint: String,
    pub pool_id: String,
    pub pool_generation: u64,
    pub placement: SandboxExecutorPlacement,
    pub placement_policy_revision: u64,
    pub capability_routes_verified: bool,
    pub smoke_passed: bool,
    pub smoke_binding_id: String,
    pub smoke_output_digest: String,
    pub evaluated_at: DateTime<Utc>,
    pub valid_until: DateTime<Utc>,
}

impl ExecutorSmokeReceipt {
    pub fn is_valid_for_at(
        &self,
        pool: &ExecutorPoolIdentity,
        policy: &OwnerPlacementPolicy,
        now: DateTime<Utc>,
    ) -> bool {
        let active_fp = pool.fingerprint.compute_digest();
        self.smoke_passed
            && self.capability_routes_verified
            && !self.smoke_binding_id.trim().is_empty()
            && valid_digest(&self.smoke_output_digest)
            && self.runtime_fingerprint == active_fp
            && self.pool_generation == pool.pool_generation
            && self.pool_id == pool.pool_id
            && self.placement == pool.placement
            && self.placement_policy_revision == policy.policy_revision
            && policy.enforce(pool).is_ok()
            && self.valid_until > self.evaluated_at
            && self.evaluated_at <= now
            && now < self.valid_until
    }

    pub fn is_valid_for(&self, pool: &ExecutorPoolIdentity, policy: &OwnerPlacementPolicy) -> bool {
        self.is_valid_for_at(pool, policy, Utc::now())
    }
}

/// Exact installed release for readiness evaluation. The descriptor and
/// payload digest are redundant projections of the installed artifact and
/// are cross-checked before any execution; callers cannot substitute a
/// different descriptor for the artifact that will run.
#[derive(Debug, Clone)]
pub struct ReleaseReadinessTarget {
    pub release_digest: String,
    pub descriptor: ModuleArtifactDescriptor,
    pub payload_digest: String,
    pub installed_artifact: InstalledModuleArtifact,
}

impl ReleaseReadinessTarget {
    fn validate(&self) -> Result<(), ExecutorReadinessError> {
        self.descriptor.validate().map_err(|error| {
            ExecutorReadinessError::InvalidCommand(format!(
                "readiness descriptor is invalid: {error}"
            ))
        })?;
        self.installed_artifact
            .descriptor
            .validate()
            .map_err(|error| {
                ExecutorReadinessError::InvalidCommand(format!(
                    "installed artifact descriptor is invalid: {error}"
                ))
            })?;
        if self.installed_artifact.installation_id.is_nil()
            || !valid_digest(&self.release_digest)
            || !valid_digest(&self.payload_digest)
            || self.release_digest != self.installed_artifact.reference.digest
            || self.payload_digest != self.descriptor.artifact_digest
            || self.descriptor != self.installed_artifact.descriptor
            || self.installed_artifact.release != self.descriptor.release_ref()
        {
            return Err(ExecutorReadinessError::InvalidCommand(
                "readiness target does not identify the exact installed artifact".to_string(),
            ));
        }
        Ok(())
    }
}

/// Authenticated command to evaluate readiness for an installation candidate
/// or predecessor. It deliberately contains no pool identity, route list, or
/// smoke outcome: those facts belong to the host-composed probe and to the
/// execution it actually performs.
#[derive(Debug, Clone)]
pub struct EvaluateReadinessCommand {
    pub operation_id: Uuid,
    pub target: ReleaseReadinessTarget,
    pub policy: OwnerPlacementPolicy,
    pub context: ModuleCommandContext,
    /// Authenticated host fact that the actor passed the platform
    /// `modules:manage` authorization boundary.
    pub actor_can_manage_modules: bool,
}

/// One owner-selected smoke execution requested from the composed executor
/// pool. The binding is selected by [`ExecutorReadinessService`], never by
/// the command caller or by artifact-controlled input.
#[derive(Debug, Clone, Copy)]
pub struct ExecutorSmokeProbeRequest<'a> {
    pub operation_id: Uuid,
    pub artifact: &'a InstalledModuleArtifact,
    pub binding: &'a ModuleRuntimeBinding,
    pub context: &'a ModuleCommandContext,
}

/// Evidence returned only after the probe actually crossed its execution
/// boundary. Output bytes are not persisted; the receipt binds their digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutorSmokeProbeOutcome {
    pub binding_id: String,
    pub output_digest: String,
}

/// Host-composed executor-pool port for readiness. Implementations own the
/// pool identity and capability broker composition, so callers cannot mint
/// a receipt by asserting a pool, route names, or `smoke_passed = true`.
#[async_trait]
pub trait ExecutorReadinessProbe: Send + Sync {
    fn pool_identity(&self) -> Result<ExecutorPoolIdentity, ExecutorReadinessError>;

    fn supports_payload_kind(&self, payload_kind: ArtifactPayloadKind) -> bool;

    fn has_capability_route(&self, capability: &str) -> bool;

    async fn execute_smoke(
        &self,
        request: ExecutorSmokeProbeRequest<'_>,
    ) -> Result<ExecutorSmokeProbeOutcome, ExecutorReadinessError>;
}

fn select_smoke_binding(
    descriptor: &ModuleArtifactDescriptor,
) -> Result<&ModuleRuntimeBinding, ExecutorReadinessError> {
    for kind in [
        ModuleRuntimeBindingKind::ActivationSmoke,
        ModuleRuntimeBindingKind::Readiness,
    ] {
        let mut matches = descriptor
            .bindings
            .iter()
            .filter(|binding| binding.kind == kind);
        let Some(binding) = matches.next() else {
            continue;
        };
        if matches.next().is_some() {
            return Err(ExecutorReadinessError::SmokeExecutionFailed(format!(
                "release `{}` declares multiple {kind:?} bindings; exactly one is admissible as a smoke probe",
                descriptor.artifact_digest
            )));
        }
        if !binding.capabilities.is_empty() {
            return Err(ExecutorReadinessError::SmokeExecutionFailed(format!(
                "smoke binding `{}` declares capabilities; the canonical zero-grant probe must be capability-free",
                binding.id
            )));
        }
        return Ok(binding);
    }
    Err(ExecutorReadinessError::SmokeExecutionFailed(format!(
        "release `{}` declares no ActivationSmoke or Readiness binding for executor readiness",
        descriptor.artifact_digest
    )))
}

/// Core service for authenticated prefetch, verified caching, placement enforcement,
/// and smoke readiness evaluation across executor pools and generations.
#[derive(Clone)]
pub struct ExecutorReadinessService {
    db: DatabaseConnection,
    blobs: Arc<dyn ArtifactBlobStore>,
    probe: Arc<dyn ExecutorReadinessProbe>,
    cache: Arc<VerifiedPayloadCache>,
    infrastructure: ControlPlaneInfrastructure,
    receipt_ttl_seconds: u32,
}

impl ExecutorReadinessService {
    pub fn new(
        db: DatabaseConnection,
        blobs: Arc<dyn ArtifactBlobStore>,
        probe: Arc<dyn ExecutorReadinessProbe>,
    ) -> Self {
        Self {
            db,
            blobs,
            probe,
            cache: Arc::new(VerifiedPayloadCache::new()),
            infrastructure: ControlPlaneInfrastructure::default(),
            receipt_ttl_seconds: DEFAULT_EXECUTOR_READINESS_RECEIPT_TTL_SECONDS,
        }
    }

    pub fn with_receipt_ttl_seconds(mut self, receipt_ttl_seconds: u32) -> Self {
        self.receipt_ttl_seconds = receipt_ttl_seconds;
        self
    }

    pub fn with_cache(mut self, cache: Arc<VerifiedPayloadCache>) -> Self {
        self.cache = cache;
        self
    }

    pub fn with_infrastructure(mut self, infrastructure: ControlPlaneInfrastructure) -> Self {
        self.infrastructure = infrastructure;
        self
    }

    pub fn cache(&self) -> &Arc<VerifiedPayloadCache> {
        &self.cache
    }

    /// Fetches the payload from platform CAS with SHA-256 verification and caches it
    /// under `(payload_digest, runtime_fingerprint)`.
    pub async fn fetch_and_verify_payload(
        &self,
        payload_digest: &str,
        fingerprint: &RuntimeFingerprint,
    ) -> Result<Vec<u8>, ExecutorReadinessError> {
        fingerprint.validate()?;
        let fp_digest = fingerprint.compute_digest();

        if let Some(cached) = self.cache.get(payload_digest, &fp_digest) {
            return Ok(cached.payload_bytes);
        }

        let blob = self.blobs.get_verified(payload_digest).await?;
        let computed = sha256_digest(&blob);
        if computed != payload_digest {
            return Err(ExecutorReadinessError::BlobStore(
                ModuleInstallationError::PayloadDigestMismatch {
                    expected: payload_digest.to_string(),
                    actual: computed,
                },
            ));
        }

        let entry = CachedPreparedPayload {
            payload_digest: payload_digest.to_string(),
            runtime_fingerprint: fp_digest,
            payload_bytes: blob.clone(),
            verified_at: self.infrastructure.now(),
        };
        self.cache.put(entry);

        Ok(blob)
    }

    /// Verifies capability routes required by all bindings in the descriptor against
    /// the host router.
    pub fn verify_capability_routes(
        &self,
        descriptor: &ModuleArtifactDescriptor,
    ) -> Result<(), ExecutorReadinessError> {
        for binding in &descriptor.bindings {
            for cap in &binding.capabilities {
                if !self.probe.has_capability_route(cap.as_str()) {
                    return Err(ExecutorReadinessError::MissingCapabilityRoute(format!(
                        "binding `{}` declares capability `{}` which is not available in host broker routes",
                        binding.id, cap
                    )));
                }
            }
        }
        Ok(())
    }

    /// Evaluates readiness for an installation candidate or predecessor release.
    ///
    /// The receipt is a by-product of execution: this method validates the
    /// authenticated command and exact installed artifact, obtains the pool
    /// identity from the host-composed probe, checks placement and routes,
    /// selects the sole host-approved smoke/readiness binding, executes it,
    /// and only then persists short-lived evidence bound to the release,
    /// payload, pool fingerprint/generation, placement, and policy revision.
    pub async fn evaluate_readiness(
        &self,
        command: EvaluateReadinessCommand,
    ) -> Result<ExecutorSmokeReceipt, ExecutorReadinessError> {
        if !(1..=MAX_EXECUTOR_READINESS_RECEIPT_TTL_SECONDS).contains(&self.receipt_ttl_seconds) {
            return Err(ExecutorReadinessError::InvalidConfiguration(format!(
                "receipt TTL must be between 1 and {MAX_EXECUTOR_READINESS_RECEIPT_TTL_SECONDS} seconds"
            )));
        }
        if command.operation_id.is_nil() {
            return Err(ExecutorReadinessError::InvalidCommand(
                "operation identity must not be nil".to_string(),
            ));
        }
        command
            .context
            .validate()
            .map_err(|error| ExecutorReadinessError::InvalidCommand(error.to_string()))?;
        if !command.actor_can_manage_modules {
            return Err(ExecutorReadinessError::AuthorizationDenied);
        }
        command.target.validate()?;
        if !command
            .target
            .installed_artifact
            .scope
            .matches_command_context(&command.context)
        {
            return Err(ExecutorReadinessError::InvalidCommand(
                "command context does not match the installed artifact scope".to_string(),
            ));
        }

        let pool = self.probe.pool_identity()?;
        pool.validate()?;
        command.policy.enforce(&pool)?;
        if !self
            .probe
            .supports_payload_kind(command.target.descriptor.payload_kind)
        {
            return Err(ExecutorReadinessError::UnsupportedPayloadKind(
                command.target.descriptor.payload_kind,
            ));
        }
        self.verify_capability_routes(&command.target.descriptor)?;
        let binding = select_smoke_binding(&command.target.descriptor)?;

        let fp_digest = pool.fingerprint.compute_digest();
        self.fetch_and_verify_payload(&command.target.payload_digest, &pool.fingerprint)
            .await?;

        let outcome = self
            .probe
            .execute_smoke(ExecutorSmokeProbeRequest {
                operation_id: command.operation_id,
                artifact: &command.target.installed_artifact,
                binding,
                context: &command.context,
            })
            .await?;
        if outcome.binding_id != binding.id || !valid_digest(&outcome.output_digest) {
            return Err(ExecutorReadinessError::SmokeExecutionFailed(
                "executor probe returned evidence for a different smoke binding".to_string(),
            ));
        }

        let evaluated_at = self.infrastructure.now();
        let valid_until = evaluated_at
            .checked_add_signed(Duration::seconds(i64::from(self.receipt_ttl_seconds)))
            .ok_or_else(|| {
                ExecutorReadinessError::InvalidConfiguration(
                    "receipt validity window exceeds the supported timestamp range".to_string(),
                )
            })?;
        let receipt = ExecutorSmokeReceipt {
            id: self.infrastructure.new_id(),
            operation_id: command.operation_id,
            installation_id: command.target.installed_artifact.installation_id,
            release_digest: command.target.release_digest,
            payload_digest: command.target.payload_digest,
            runtime_fingerprint: fp_digest,
            pool_id: pool.pool_id,
            pool_generation: pool.pool_generation,
            placement: pool.placement,
            placement_policy_revision: command.policy.policy_revision,
            capability_routes_verified: true,
            smoke_passed: true,
            smoke_binding_id: outcome.binding_id,
            smoke_output_digest: outcome.output_digest,
            evaluated_at,
            valid_until,
        };

        self.persist_receipt(&receipt).await?;

        Ok(receipt)
    }

    async fn persist_receipt(
        &self,
        receipt: &ExecutorSmokeReceipt,
    ) -> Result<(), ExecutorReadinessError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let placement_str = match receipt.placement {
            SandboxExecutorPlacement::InProcess => "in_process",
            SandboxExecutorPlacement::IsolatedWorker => "isolated_worker",
        };
        let placeholders = (1..=16)
            .map(|position| backend.placeholder(position))
            .collect::<Vec<_>>()
            .join(", ");
        let statement = Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_executor_readiness_receipts (\
                    id, operation_id, installation_id, release_digest, payload_digest, \
                    runtime_fingerprint, pool_id, pool_generation, placement, \
                    placement_policy_revision, capability_routes_verified, smoke_passed, \
                    smoke_binding_id, smoke_output_digest, evaluated_at, valid_until\
                 ) VALUES ({placeholders})"
            ),
            vec![
                backend.uuid_value(receipt.id),
                backend.uuid_value(receipt.operation_id),
                backend.uuid_value(receipt.installation_id),
                receipt.release_digest.clone().into(),
                receipt.payload_digest.clone().into(),
                receipt.runtime_fingerprint.clone().into(),
                receipt.pool_id.clone().into(),
                i64::try_from(receipt.pool_generation)
                    .map_err(|_| {
                        ExecutorReadinessError::Store(
                            "pool generation exceeds the database range".to_string(),
                        )
                    })?
                    .into(),
                placement_str.into(),
                i64::try_from(receipt.placement_policy_revision)
                    .map_err(|_| {
                        ExecutorReadinessError::Store(
                            "placement policy revision exceeds the database range".to_string(),
                        )
                    })?
                    .into(),
                backend.bool_value(receipt.capability_routes_verified),
                backend.bool_value(receipt.smoke_passed),
                receipt.smoke_binding_id.clone().into(),
                receipt.smoke_output_digest.clone().into(),
                backend.datetime_value(receipt.evaluated_at),
                backend.datetime_value(receipt.valid_until),
            ],
        );

        self.db
            .execute_raw(statement)
            .await
            .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;

        Ok(())
    }

    /// Queries the latest durable readiness receipt for a release on a given pool and generation.
    pub async fn get_receipt(
        &self,
        release_digest: &str,
        pool_id: &str,
        pool_generation: u64,
        runtime_fingerprint: &str,
    ) -> Result<Option<ExecutorSmokeReceipt>, ExecutorReadinessError> {
        let backend = SupportedBackend::new(self.db.get_database_backend())
            .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let statement = Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT id, operation_id, installation_id, release_digest, payload_digest, \
                        runtime_fingerprint, pool_id, pool_generation, placement, \
                        placement_policy_revision, capability_routes_verified, smoke_passed, \
                        smoke_binding_id, smoke_output_digest, evaluated_at, valid_until \
                 FROM module_executor_readiness_receipts \
                 WHERE release_digest = {} AND pool_id = {} AND pool_generation = {} \
                   AND runtime_fingerprint = {} \
                 ORDER BY evaluated_at DESC, id DESC LIMIT 1",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
            ),
            vec![
                release_digest.to_owned().into(),
                pool_id.to_owned().into(),
                i64::try_from(pool_generation)
                    .map_err(|_| {
                        ExecutorReadinessError::Store(
                            "pool generation exceeds the database range".to_string(),
                        )
                    })?
                    .into(),
                runtime_fingerprint.to_owned().into(),
            ],
        );

        let row = self
            .db
            .query_one_raw(statement)
            .await
            .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let placement_str: String = row
            .try_get("", "placement")
            .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let pool_gen: i64 = row
            .try_get("", "pool_generation")
            .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let pol_rev: i64 = row
            .try_get("", "placement_policy_revision")
            .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let placement = match placement_str.as_str() {
            "in_process" => SandboxExecutorPlacement::InProcess,
            "isolated_worker" => SandboxExecutorPlacement::IsolatedWorker,
            other => {
                return Err(ExecutorReadinessError::Store(format!(
                    "unknown executor placement `{other}` in readiness receipt"
                )));
            }
        };

        Ok(Some(ExecutorSmokeReceipt {
            id: backend
                .uuid_from_row(&row, "id")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            operation_id: backend
                .uuid_from_row(&row, "operation_id")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            installation_id: backend
                .uuid_from_row(&row, "installation_id")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            release_digest: row
                .try_get("", "release_digest")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            payload_digest: row
                .try_get("", "payload_digest")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            runtime_fingerprint: row
                .try_get("", "runtime_fingerprint")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            pool_id: row
                .try_get("", "pool_id")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            pool_generation: u64::try_from(pool_gen).map_err(|_| {
                ExecutorReadinessError::Store(
                    "readiness receipt pool generation is outside the supported range".to_string(),
                )
            })?,
            placement,
            placement_policy_revision: u64::try_from(pol_rev).map_err(|_| {
                ExecutorReadinessError::Store(
                    "readiness receipt placement policy revision is outside the supported range"
                        .to_string(),
                )
            })?,
            capability_routes_verified: backend
                .bool_from_row(&row, "capability_routes_verified")
                .map_err(|e| {
                ExecutorReadinessError::Store(e.to_string())
            })?,
            smoke_passed: backend
                .bool_from_row(&row, "smoke_passed")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            smoke_binding_id: row
                .try_get("", "smoke_binding_id")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            smoke_output_digest: row
                .try_get("", "smoke_output_digest")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            evaluated_at: backend
                .datetime_from_row(&row, "evaluated_at")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            valid_until: backend
                .datetime_from_row(&row, "valid_until")
                .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
        }))
    }

    /// Evaluates eligibility for automatic mode across every serving and recovery pool.
    ///
    /// Automatic mode requires BOTH candidate and predecessor to pass smoke readiness
    /// on every executor fingerprint and generation that may serve or recover the operation.
    /// If either release lacks a valid, current smoke receipt on any required pool,
    /// automatic mode is strictly denied.
    pub async fn check_automatic_mode_eligibility(
        &self,
        candidate_release_digest: &str,
        predecessor_release_digest: Option<&str>,
        pools: &[ExecutorPoolIdentity],
        policy: &OwnerPlacementPolicy,
    ) -> Result<(), ExecutorReadinessError> {
        if pools.is_empty() {
            return Err(ExecutorReadinessError::AutomaticModeDenied(
                "no executor pools configured for serving or recovery".to_string(),
            ));
        }

        let now = self.infrastructure.now();
        for pool in pools {
            pool.validate()?;
            policy.enforce(pool)?;
            let fp = pool.fingerprint.compute_digest();

            // Candidate check
            let candidate_receipt = self
                .get_receipt(
                    candidate_release_digest,
                    &pool.pool_id,
                    pool.pool_generation,
                    &fp,
                )
                .await?;
            match candidate_receipt {
                Some(r) if r.is_valid_for_at(pool, policy, now) => {}
                Some(_) => {
                    return Err(ExecutorReadinessError::AutomaticModeDenied(format!(
                        "candidate `{candidate_release_digest}` has invalid or failed smoke receipt on pool `{}` generation {}",
                        pool.pool_id, pool.pool_generation
                    )));
                }
                None => {
                    return Err(ExecutorReadinessError::AutomaticModeDenied(format!(
                        "candidate `{candidate_release_digest}` lacks smoke readiness receipt on pool `{}` generation {}",
                        pool.pool_id, pool.pool_generation
                    )));
                }
            }

            // Predecessor check (if updating an existing installation)
            if let Some(pred_digest) = predecessor_release_digest {
                let pred_receipt = self
                    .get_receipt(pred_digest, &pool.pool_id, pool.pool_generation, &fp)
                    .await?;
                match pred_receipt {
                    Some(r) if r.is_valid_for_at(pool, policy, now) => {}
                    Some(_) => {
                        return Err(ExecutorReadinessError::AutomaticModeDenied(format!(
                            "predecessor `{pred_digest}` has invalid or failed smoke receipt on pool `{}` generation {} (rollback readiness unproven)",
                            pool.pool_id, pool.pool_generation
                        )));
                    }
                    None => {
                        return Err(ExecutorReadinessError::AutomaticModeDenied(format!(
                            "predecessor `{pred_digest}` lacks smoke readiness receipt on pool `{}` generation {} (rollback readiness unproven)",
                            pool.pool_id, pool.pool_generation
                        )));
                    }
                }
            }
        }

        Ok(())
    }
}
