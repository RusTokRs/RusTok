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
    collections::{HashMap, HashSet},
    sync::{Arc, RwLock},
};

use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use rustok_sandbox::SandboxExecutorPlacement;

use crate::{
    ArtifactBlobStore, ControlPlaneInfrastructure, InstalledModuleArtifact,
    ModuleArtifactDescriptor, ModuleInstallationError,
};

fn sha256_digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

fn valid_digest(digest: &str) -> bool {
    digest.starts_with("sha256:")
        && digest.len() == 71
        && digest[7..].chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, thiserror::Error)]
pub enum ExecutorReadinessError {
    #[error("Invalid runtime fingerprint: {0}")]
    InvalidFingerprint(String),
    #[error("Invalid pool identity: {0}")]
    InvalidPoolIdentity(String),
    #[error("Isolated worker execution is required by owner policy, but pool placement is in-process (in-process fallback is strictly prohibited)")]
    IsolatedWorkerRequired,
    #[error("Missing or invalid placement attestation for isolated worker pool")]
    MissingWorkerAttestation,
    #[error("Missing declared capability route: {0}")]
    MissingCapabilityRoute(String),
    #[error("Engine or configuration changed: previous readiness receipt invalidated (receipt fingerprint `{0}` != active fingerprint `{1}`)")]
    EngineFingerprintMismatch(String, String),
    #[error("Stale pool generation: receipt generation {0} does not match current pool generation {1} (smoke readiness must repeat on generation change)")]
    StalePoolGeneration(u64, u64),
    #[error("Smoke binding execution failed: {0}")]
    SmokeExecutionFailed(String),
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
                self.isolated_worker_image_digest.as_deref().unwrap_or("none")
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
        if let Some(ref image) = self.isolated_worker_image_digest {
            if !valid_digest(image) {
                return Err(ExecutorReadinessError::InvalidFingerprint(
                    "isolated_worker_image_digest must be a valid sha256 digest".to_string(),
                ));
            }
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

/// Thread-safe verified payload cache keyed by `(payload_digest, runtime_fingerprint)`.
#[derive(Debug, Default)]
pub struct VerifiedPayloadCache {
    entries: RwLock<HashMap<(String, String), CachedPreparedPayload>>,
}

impl VerifiedPayloadCache {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, payload_digest: &str, fingerprint_digest: &str) -> Option<CachedPreparedPayload> {
        self.entries
            .read()
            .unwrap()
            .get(&(payload_digest.to_string(), fingerprint_digest.to_string()))
            .cloned()
    }

    pub fn put(&self, entry: CachedPreparedPayload) {
        let key = (entry.payload_digest.clone(), entry.runtime_fingerprint.clone());
        self.entries.write().unwrap().insert(key, entry);
    }

    pub fn invalidate_fingerprint(&self, fingerprint_digest: &str) {
        self.entries
            .write()
            .unwrap()
            .retain(|(_, fp), _| fp != fingerprint_digest);
    }

    pub fn clear(&self) {
        self.entries.write().unwrap().clear();
    }

    pub fn len(&self) -> usize {
        self.entries.read().unwrap().len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.read().unwrap().is_empty()
    }
}

/// Durable receipt recording successful smoke readiness on a specific executor fingerprint and pool generation.
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
    pub evaluated_at: DateTime<Utc>,
}

impl ExecutorSmokeReceipt {
    pub fn is_valid_for(&self, pool: &ExecutorPoolIdentity) -> bool {
        let active_fp = pool.fingerprint.compute_digest();
        self.smoke_passed
            && self.capability_routes_verified
            && self.runtime_fingerprint == active_fp
            && self.pool_generation == pool.pool_generation
            && self.pool_id == pool.pool_id
    }
}

/// Target release for readiness evaluation.
#[derive(Debug, Clone)]
pub struct ReleaseReadinessTarget {
    pub release_digest: String,
    pub descriptor: ModuleArtifactDescriptor,
    pub payload_digest: String,
    pub installed_artifact: Option<InstalledModuleArtifact>,
}

/// Command to evaluate readiness for an installation candidate or predecessor.
#[derive(Debug, Clone)]
pub struct EvaluateReadinessCommand {
    pub operation_id: Uuid,
    pub installation_id: Uuid,
    pub target: ReleaseReadinessTarget,
    pub pool: ExecutorPoolIdentity,
    pub policy: OwnerPlacementPolicy,
    pub smoke_test_passed: bool,
}

/// Core service for authenticated prefetch, verified caching, placement enforcement,
/// and smoke readiness evaluation across executor pools and generations.
#[derive(Clone)]
pub struct ExecutorReadinessService {
    db: DatabaseConnection,
    blobs: Arc<dyn ArtifactBlobStore>,
    cache: Arc<VerifiedPayloadCache>,
    available_routes: Arc<HashSet<String>>,
    infrastructure: ControlPlaneInfrastructure,
}

impl ExecutorReadinessService {
    pub fn new(db: DatabaseConnection, blobs: Arc<dyn ArtifactBlobStore>) -> Self {
        Self {
            db,
            blobs,
            cache: Arc::new(VerifiedPayloadCache::new()),
            available_routes: Arc::new(HashSet::new()),
            infrastructure: ControlPlaneInfrastructure::default(),
        }
    }

    pub fn with_routes(mut self, routes: HashSet<String>) -> Self {
        self.available_routes = Arc::new(routes);
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
            verified_at: Utc::now(),
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
                if !self.available_routes.contains(cap.as_str()) {
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
    /// Validates pool identity, owner placement policy (zero in-process fallback),
    /// capability routes, fetches/verifies payload into cache, validates smoke test
    /// results, and persists the durable readiness receipt.
    pub async fn evaluate_readiness(
        &self,
        command: EvaluateReadinessCommand,
    ) -> Result<ExecutorSmokeReceipt, ExecutorReadinessError> {
        command.pool.validate()?;
        command.policy.enforce(&command.pool)?;
        self.verify_capability_routes(&command.target.descriptor)?;

        let fp_digest = command.pool.fingerprint.compute_digest();
        self.fetch_and_verify_payload(&command.target.payload_digest, &command.pool.fingerprint)
            .await?;

        if !command.smoke_test_passed {
            return Err(ExecutorReadinessError::SmokeExecutionFailed(format!(
                "smoke binding probe failed for release `{}` on pool `{}`",
                command.target.release_digest, command.pool.pool_id
            )));
        }

        let receipt = ExecutorSmokeReceipt {
            id: Uuid::new_v4(),
            operation_id: command.operation_id,
            installation_id: command.installation_id,
            release_digest: command.target.release_digest,
            payload_digest: command.target.payload_digest,
            runtime_fingerprint: fp_digest,
            pool_id: command.pool.pool_id,
            pool_generation: command.pool.pool_generation,
            placement: command.pool.placement,
            placement_policy_revision: command.policy.policy_revision,
            capability_routes_verified: true,
            smoke_passed: true,
            evaluated_at: Utc::now(),
        };

        self.persist_receipt(&receipt).await?;

        Ok(receipt)
    }

    async fn persist_receipt(
        &self,
        receipt: &ExecutorSmokeReceipt,
    ) -> Result<(), ExecutorReadinessError> {
        let backend = self.db.get_database_backend();
        let placement_str = match receipt.placement {
            SandboxExecutorPlacement::InProcess => "in_process",
            SandboxExecutorPlacement::IsolatedWorker => "isolated_worker",
        };

        let statement = match backend {
            DbBackend::Postgres => Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO module_executor_readiness_receipts (\
                    id, operation_id, installation_id, release_digest, payload_digest,\
                    runtime_fingerprint, pool_id, pool_generation, placement,\
                    placement_policy_revision, capability_routes_verified, smoke_passed,\
                    evaluated_at\
                 ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)",
                vec![
                    receipt.id.into(),
                    receipt.operation_id.into(),
                    receipt.installation_id.into(),
                    receipt.release_digest.clone().into(),
                    receipt.payload_digest.clone().into(),
                    receipt.runtime_fingerprint.clone().into(),
                    receipt.pool_id.clone().into(),
                    (receipt.pool_generation as i64).into(),
                    placement_str.into(),
                    (receipt.placement_policy_revision as i64).into(),
                    receipt.capability_routes_verified.into(),
                    receipt.smoke_passed.into(),
                    receipt.evaluated_at.into(),
                ],
            ),
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_executor_readiness_receipts (\
                    id, operation_id, installation_id, release_digest, payload_digest,\
                    runtime_fingerprint, pool_id, pool_generation, placement,\
                    placement_policy_revision, capability_routes_verified, smoke_passed,\
                    evaluated_at\
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
                vec![
                    receipt.id.to_string().into(),
                    receipt.operation_id.to_string().into(),
                    receipt.installation_id.to_string().into(),
                    receipt.release_digest.clone().into(),
                    receipt.payload_digest.clone().into(),
                    receipt.runtime_fingerprint.clone().into(),
                    receipt.pool_id.clone().into(),
                    (receipt.pool_generation as i64).into(),
                    placement_str.into(),
                    (receipt.placement_policy_revision as i64).into(),
                    (if receipt.capability_routes_verified { 1i32 } else { 0i32 }).into(),
                    (if receipt.smoke_passed { 1i32 } else { 0i32 }).into(),
                    receipt.evaluated_at.to_rfc3339().into(),
                ],
            ),
            _ => {
                return Err(ExecutorReadinessError::Store(
                    "Unsupported database backend".to_string(),
                ))
            }
        };

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
        let backend = self.db.get_database_backend();
        let statement = match backend {
            DbBackend::Postgres => Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT id, operation_id, installation_id, release_digest, payload_digest,\
                        runtime_fingerprint, pool_id, pool_generation, placement,\
                        placement_policy_revision, capability_routes_verified, smoke_passed,\
                        evaluated_at \
                 FROM module_executor_readiness_receipts \
                 WHERE release_digest = $1 AND pool_id = $2 AND pool_generation = $3 \
                   AND runtime_fingerprint = $4 \
                 ORDER BY evaluated_at DESC LIMIT 1",
                vec![
                    release_digest.to_owned().into(),
                    pool_id.to_owned().into(),
                    (pool_generation as i64).into(),
                    runtime_fingerprint.to_owned().into(),
                ],
            ),
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT id, operation_id, installation_id, release_digest, payload_digest,\
                        runtime_fingerprint, pool_id, pool_generation, placement,\
                        placement_policy_revision, capability_routes_verified, smoke_passed,\
                        evaluated_at \
                 FROM module_executor_readiness_receipts \
                 WHERE release_digest = ?1 AND pool_id = ?2 AND pool_generation = ?3 \
                   AND runtime_fingerprint = ?4 \
                 ORDER BY evaluated_at DESC LIMIT 1",
                vec![
                    release_digest.to_owned().into(),
                    pool_id.to_owned().into(),
                    (pool_generation as i64).into(),
                    runtime_fingerprint.to_owned().into(),
                ],
            ),
            _ => {
                return Err(ExecutorReadinessError::Store(
                    "Unsupported database backend".to_string(),
                ))
            }
        };

        let row = self
            .db
            .query_one_raw(statement)
            .await
            .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let id_str: String = row.try_get("", "id").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let op_str: String = row.try_get("", "operation_id").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let inst_str: String = row.try_get("", "installation_id").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let placement_str: String = row.try_get("", "placement").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let pool_gen: i64 = row.try_get("", "pool_generation").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let pol_rev: i64 = row.try_get("", "placement_policy_revision").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
        let routes_verified = match backend {
            DbBackend::Postgres => row.try_get::<bool>("", "capability_routes_verified").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            _ => row.try_get::<i32>("", "capability_routes_verified").map_err(|e| ExecutorReadinessError::Store(e.to_string()))? != 0,
        };
        let smoke_passed = match backend {
            DbBackend::Postgres => row.try_get::<bool>("", "smoke_passed").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            _ => row.try_get::<i32>("", "smoke_passed").map_err(|e| ExecutorReadinessError::Store(e.to_string()))? != 0,
        };
        let evaluated_at = match backend {
            DbBackend::Postgres => row.try_get::<DateTime<Utc>>("", "evaluated_at").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            _ => {
                let s: String = row.try_get("", "evaluated_at").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?;
                DateTime::parse_from_rfc3339(&s)
                    .map_err(|e| ExecutorReadinessError::Store(e.to_string()))?
                    .with_timezone(&Utc)
            }
        };

        let placement = match placement_str.as_str() {
            "isolated_worker" => SandboxExecutorPlacement::IsolatedWorker,
            _ => SandboxExecutorPlacement::InProcess,
        };

        Ok(Some(ExecutorSmokeReceipt {
            id: Uuid::parse_str(&id_str).map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            operation_id: Uuid::parse_str(&op_str).map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            installation_id: Uuid::parse_str(&inst_str).map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            release_digest: row.try_get("", "release_digest").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            payload_digest: row.try_get("", "payload_digest").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            runtime_fingerprint: row.try_get("", "runtime_fingerprint").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            pool_id: row.try_get("", "pool_id").map_err(|e| ExecutorReadinessError::Store(e.to_string()))?,
            pool_generation: pool_gen as u64,
            placement,
            placement_policy_revision: pol_rev as u64,
            capability_routes_verified: routes_verified,
            smoke_passed,
            evaluated_at,
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
    ) -> Result<(), ExecutorReadinessError> {
        if pools.is_empty() {
            return Err(ExecutorReadinessError::AutomaticModeDenied(
                "no executor pools configured for serving or recovery".to_string(),
            ));
        }

        for pool in pools {
            pool.validate()?;
            let fp = pool.fingerprint.compute_digest();

            // Candidate check
            let candidate_receipt = self
                .get_receipt(candidate_release_digest, &pool.pool_id, pool.pool_generation, &fp)
                .await?;
            match candidate_receipt {
                Some(r) if r.is_valid_for(pool) => {}
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
                    Some(r) if r.is_valid_for(pool) => {}
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
