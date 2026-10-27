use sea_orm::DatabaseConnection;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::task::JoinHandle;
use tracing::{error, info};
use uuid::Uuid;

use rustok_modules::{
    AdmissionReverificationPolicyRevision, ArtifactAdmissionReverificationCommand,
    ModuleCommandContext, ModuleControlPlane, ModuleInstallationError, ModuleInstallationScope,
    TrustVerifier,
};
use rustok_storage::StorageRuntime;
use rustok_verification_transport::GrpcTrustVerifier;
use rustok_worker_transport::MutualTlsClientConfig;

static MODULE_ARTIFACT_MAINTENANCE_INSTANCE_IDS: AtomicU64 = AtomicU64::new(1);

/// Bounded per-pass budgets. A pass repeats on the poll interval, so these
/// bound one tick's database, storage, and verifier work instead of total
/// maintenance progress.
const DATA_OBJECT_GC_TENANT_SCAN_LIMIT: u32 = 256;
const DATA_OBJECT_GC_CANDIDATES_PER_TENANT: u32 = 100;
const ADMISSION_REVERIFICATIONS_PER_PASS: u32 = 32;

pub struct ModuleArtifactMaintenanceHandle {
    pub instance_id: u64,
    pub join_handle: JoinHandle<()>,
}

impl ModuleArtifactMaintenanceHandle {
    pub fn instance_id(&self) -> u64 {
        self.instance_id
    }

    pub fn is_finished(&self) -> bool {
        self.join_handle.is_finished()
    }

    pub fn abort(&self) {
        self.join_handle.abort();
    }

    pub async fn join(self) -> std::result::Result<(), tokio::task::JoinError> {
        self.join_handle.await
    }
}

/// Deployment-owned configuration for the admission-reverification leg.
/// The leg exists only when the operator configures a verification
/// endpoint; pointing it at a worker is an explicit deployment decision,
/// and invalid configuration disables the leg loudly at startup rather
/// than letting stale evidence pass for fresh.
struct AdmissionReverificationConfig {
    endpoint: String,
    tls: MutualTlsClientConfig,
    trust_policy: AdmissionReverificationPolicyRevision,
    actor_id: Uuid,
}

fn admission_reverification_config_from_env()
-> Option<Result<AdmissionReverificationConfig, String>> {
    let endpoint = match std::env::var("RUSTOK_MODULE_MAINTENANCE_VERIFICATION_ENDPOINT") {
        Ok(endpoint) => endpoint,
        Err(std::env::VarError::NotPresent) => return None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Some(Err(
                "RUSTOK_MODULE_MAINTENANCE_VERIFICATION_ENDPOINT must be valid Unicode"
                    .to_string(),
            ));
        }
    };
    let configured = (|| {
        if !endpoint.starts_with("https://") {
            return Err(
                "RUSTOK_MODULE_MAINTENANCE_VERIFICATION_ENDPOINT must use an https:// endpoint for mTLS"
                    .to_string(),
            );
        }
        let tls = MutualTlsClientConfig::from_env_prefix("RUSTOK_MODULE_MAINTENANCE_VERIFICATION")?;
        let trust_policy = AdmissionReverificationPolicyRevision {
            trust_policy_revision: required_positive_u64_env(
                "RUSTOK_MODULE_MAINTENANCE_TRUST_POLICY_REVISION",
            )?,
            capability_policy_revision: required_positive_u64_env(
                "RUSTOK_MODULE_MAINTENANCE_CAPABILITY_POLICY_REVISION",
            )?,
        };
        let actor_id = std::env::var("RUSTOK_MODULE_MAINTENANCE_ACTOR_ID")
            .map_err(|error| match error {
                std::env::VarError::NotUnicode(_) => {
                    "RUSTOK_MODULE_MAINTENANCE_ACTOR_ID must be valid Unicode".to_string()
                }
                std::env::VarError::NotPresent => {
                    "RUSTOK_MODULE_MAINTENANCE_ACTOR_ID is required".to_string()
                }
            })
            .and_then(|value| {
                Uuid::parse_str(value.trim()).map_err(|error| {
                    format!("RUSTOK_MODULE_MAINTENANCE_ACTOR_ID is not a valid UUID: {error}")
                })
            })?;
        if actor_id.is_nil() {
            return Err("RUSTOK_MODULE_MAINTENANCE_ACTOR_ID must not be the nil UUID".to_string());
        }
        Ok(AdmissionReverificationConfig {
            endpoint,
            tls,
            trust_policy,
            actor_id,
        })
    })();
    Some(configured)
}

fn required_positive_u64_env(name: &str) -> Result<u64, String> {
    let raw = std::env::var(name).map_err(|error| match error {
        std::env::VarError::NotUnicode(_) => format!("{name} must be valid Unicode"),
        std::env::VarError::NotPresent => format!("{name} is required"),
    })?;
    let value: u64 = raw
        .trim()
        .parse()
        .map_err(|error| format!("{name} must be an unsigned integer: {error}"))?;
    if value == 0 {
        return Err(format!("{name} must be positive"));
    }
    Ok(value)
}

/// Deterministic per-candidate idempotency key: re-driving the same stale
/// admission under the same policy revisions replays the durable owner
/// command instead of paying a second verifier round-trip, while any
/// revision movement produces a fresh key because the inputs are part of
/// the digest.
fn reverification_idempotency_key(
    installation_id: Uuid,
    expected_revision: u64,
    trust_policy: &AdmissionReverificationPolicyRevision,
) -> Uuid {
    let mut hasher = Sha256::new();
    hasher.update(b"rustok:module-admission-reverification:v1\0");
    hasher.update(installation_id.as_bytes());
    hasher.update(expected_revision.to_be_bytes());
    hasher.update(trust_policy.trust_policy_revision.to_be_bytes());
    hasher.update(trust_policy.capability_policy_revision.to_be_bytes());
    let digest = hasher.finalize();
    let mut bytes = [0_u8; 16];
    bytes.copy_from_slice(&digest[..16]);
    bytes[6] = (bytes[6] & 0x0f) | 0x50;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Uuid::from_bytes(bytes)
}

async fn connect_verifier(
    config: &AdmissionReverificationConfig,
) -> Result<Arc<dyn TrustVerifier>, String> {
    let endpoint = tonic::transport::Endpoint::from_shared(config.endpoint.clone())
        .map_err(|error| error.to_string())?;
    let verifier = GrpcTrustVerifier::connect_with_tls(endpoint, config.tls.tls_config()).await?;
    verifier.check_readiness().await?;
    Ok(Arc::new(verifier))
}

/// Runs the artifact control-plane maintenance passes that no request
/// path can trigger: published-blob garbage collection, tenant
/// data-object garbage collection, and current-policy reverification of
/// admissions whose durable evidence predates the configured policy
/// revisions. Each leg is fail-isolated per pass (one failing leg logs
/// and the others still run) and fail-closed inside the crate owners
/// (no deletion or evidence refresh happens on partial knowledge).
pub fn spawn_module_artifact_maintenance_handle(
    db: DatabaseConnection,
    storage: StorageRuntime,
    poll_interval_ms: u64,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) -> ModuleArtifactMaintenanceHandle {
    let instance_id = MODULE_ARTIFACT_MAINTENANCE_INSTANCE_IDS.fetch_add(1, Ordering::Relaxed);
    let min_interval_ms = if cfg!(test) { 5 } else { 1_000 };
    let interval = Duration::from_millis(poll_interval_ms.max(min_interval_ms));

    let reverification_config = match admission_reverification_config_from_env() {
        Some(Ok(config)) => Some(config),
        Some(Err(error)) => {
            error!(
                worker = "module_artifact_maintenance",
                instance_id,
                error = %error,
                "Admission reverification is misconfigured; the leg is disabled and durable evidence will not refresh"
            );
            None
        }
        None => {
            info!(
                worker = "module_artifact_maintenance",
                instance_id,
                "Admission reverification endpoint is not configured; evidence refresh leg disabled"
            );
            None
        }
    };

    info!(
        worker = "module_artifact_maintenance",
        instance_id, poll_interval_ms, "Starting module artifact maintenance background worker"
    );

    let join_handle = tokio::spawn(async move {
        let control_plane = ModuleControlPlane::new(db);
        let mut verifier: Option<Arc<dyn TrustVerifier>> = None;
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if *stop_rx.borrow() {
                        info!(
                            worker = "module_artifact_maintenance",
                            instance_id,
                            "Module artifact maintenance received shutdown signal"
                        );
                        break;
                    }

                    // Published-blob garbage collection. The retention
                    // snapshot is reloaded per pass so holds recorded
                    // between passes protect bytes before any deletion.
                    match control_plane.artifact_blob_retention_policy().await {
                        Ok(policy) => {
                            let reconciler = control_plane.artifact_blob_reconciler(storage.clone());
                            match reconciler.reconcile_published_blobs(&policy).await {
                                Ok(report) if report.has_changes() => {
                                    info!(
                                        worker = "module_artifact_maintenance",
                                        instance_id,
                                        scanned = report.scanned,
                                        referenced = report.referenced,
                                        retained_by_policy = report.retained_by_policy,
                                        newly_marked = report.newly_marked,
                                        pending_grace = report.pending_grace,
                                        claim_in_progress = report.claim_in_progress,
                                        revoked = report.revoked,
                                        raced_candidates = report.raced_candidates,
                                        deleted = report.deleted,
                                        held_digests = policy.held_digest_count(),
                                        "Reconciled published artifact blobs"
                                    );
                                }
                                Ok(_) => {}
                                Err(err) => {
                                    error!(
                                        worker = "module_artifact_maintenance",
                                        instance_id,
                                        error = %err,
                                        "Published artifact blob reconciliation failed"
                                    );
                                }
                            }
                        }
                        Err(err) => {
                            error!(
                                worker = "module_artifact_maintenance",
                                instance_id,
                                error = %err,
                                "Loading the blob retention snapshot failed; no blob deletion this pass"
                            );
                        }
                    }

                    // Tenant data-object garbage collection. The
                    // retention-hold snapshot is loaded once for the pass
                    // (and only when at least one tenant has candidates),
                    // then reused for every tenant sweep.
                    let data_object_gc = control_plane.artifact_data_object_gc(storage.clone());
                    match data_object_gc
                        .tenants_with_candidates(DATA_OBJECT_GC_TENANT_SCAN_LIMIT)
                        .await
                    {
                        Ok(tenant_ids) if !tenant_ids.is_empty() => {
                            match data_object_gc.load_ledger_retention_snapshot().await {
                                Ok(retention) => {
                                    for tenant_id in tenant_ids {
                                        match data_object_gc
                                            .sweep_tenant_with_ledger_retention_snapshot(
                                                tenant_id,
                                                DATA_OBJECT_GC_CANDIDATES_PER_TENANT,
                                                &retention,
                                            )
                                            .await
                                        {
                                            Ok(result) if result.deleted > 0 || result.retained > 0 => {
                                                info!(
                                                    worker = "module_artifact_maintenance",
                                                    instance_id,
                                                    tenant_id = %tenant_id,
                                                    deleted = result.deleted,
                                                    retained = result.retained,
                                                    held_namespaces = retention.held_namespace_count(),
                                                    "Swept tenant artifact data objects"
                                                );
                                            }
                                            Ok(_) => {}
                                            Err(err) => {
                                                error!(
                                                    worker = "module_artifact_maintenance",
                                                    instance_id,
                                                    tenant_id = %tenant_id,
                                                    error = %err,
                                                    "Tenant artifact data object sweep failed"
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(err) => {
                                    error!(
                                        worker = "module_artifact_maintenance",
                                        instance_id,
                                        error = %err,
                                        "Loading the data-object retention snapshot failed; no data-object deletion this pass"
                                    );
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(err) => {
                            error!(
                                worker = "module_artifact_maintenance",
                                instance_id,
                                error = %err,
                                "Tenant data-object GC discovery failed"
                            );
                        }
                    }

                    // Current-policy reverification of stale admissions.
                    if let Some(config) = &reverification_config {
                        if verifier.is_none() {
                            match connect_verifier(config).await {
                                Ok(connected) => {
                                    info!(
                                        worker = "module_artifact_maintenance",
                                        instance_id,
                                        "Connected to the artifact verification worker"
                                    );
                                    verifier = Some(connected);
                                }
                                Err(err) => {
                                    error!(
                                        worker = "module_artifact_maintenance",
                                        instance_id,
                                        error = %err,
                                        "Verification worker connection failed; reverification deferred to the next pass"
                                    );
                                }
                            }
                        }
                        if let Some(verifier) = &verifier {
                            let service = control_plane.admission_reverification(
                                Arc::clone(verifier),
                                config.trust_policy,
                            );
                            match service
                                .list_stale_candidates(ADMISSION_REVERIFICATIONS_PER_PASS)
                                .await
                            {
                                Ok(candidates) => {
                                    for candidate in candidates {
                                        let idempotency_key = reverification_idempotency_key(
                                            candidate.installation_id,
                                            candidate.expected_revision,
                                            &config.trust_policy,
                                        );
                                        let context = ModuleCommandContext {
                                            actor_id: config.actor_id,
                                            tenant_id: match &candidate.scope {
                                                ModuleInstallationScope::Platform => None,
                                                ModuleInstallationScope::Tenant { tenant_id } => {
                                                    Some(*tenant_id)
                                                }
                                            },
                                            trace_id: format!(
                                                "module-maintenance:{idempotency_key}"
                                            ),
                                            correlation_id: idempotency_key,
                                            idempotency_key,
                                        };
                                        let request = ArtifactAdmissionReverificationCommand {
                                            installation_id: candidate.installation_id,
                                            scope: candidate.scope,
                                            expected_revision: candidate.expected_revision,
                                            context,
                                        };
                                        match service.reverify_admission(request).await {
                                            Ok(revision) => {
                                                info!(
                                                    worker = "module_artifact_maintenance",
                                                    instance_id,
                                                    installation_id = %candidate.installation_id,
                                                    revision,
                                                    "Reverified artifact admission under current policy"
                                                );
                                            }
                                            Err(ModuleInstallationError::AdmissionRevisionConflict(_)) => {
                                                info!(
                                                    worker = "module_artifact_maintenance",
                                                    instance_id,
                                                    installation_id = %candidate.installation_id,
                                                    "Admission changed concurrently; reverification will re-evaluate on the next pass"
                                                );
                                            }
                                            Err(err) => {
                                                error!(
                                                    worker = "module_artifact_maintenance",
                                                    instance_id,
                                                    installation_id = %candidate.installation_id,
                                                    error = %err,
                                                    "Artifact admission reverification failed"
                                                );
                                            }
                                        }
                                    }
                                }
                                Err(err) => {
                                    error!(
                                        worker = "module_artifact_maintenance",
                                        instance_id,
                                        error = %err,
                                        "Stale admission candidate scan failed"
                                    );
                                }
                            }
                        }
                    }
                }
                changed = stop_rx.changed() => {
                    if changed.is_err() || *stop_rx.borrow() {
                        info!(
                            worker = "module_artifact_maintenance",
                            instance_id,
                            "Module artifact maintenance stopping"
                        );
                        break;
                    }
                }
            }
        }
    });

    ModuleArtifactMaintenanceHandle {
        instance_id,
        join_handle,
    }
}
