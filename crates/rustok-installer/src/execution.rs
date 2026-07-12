//! Typed input and output contracts for an install-apply executor.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{InstallPlan, InstallReceipt, InstallState, InstallStep};

/// Host-selected execution options for one install apply operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallApplyOptions {
    pub lock_owner: String,
    pub lock_ttl_secs: i64,
    pub pg_admin_url: Option<String>,
}

impl Default for InstallApplyOptions {
    fn default() -> Self {
        Self {
            lock_owner: "installer".to_string(),
            lock_ttl_secs: 900,
            pg_admin_url: None,
        }
    }
}

/// Durable result of a completed install apply operation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallApplyOutput {
    pub status: String,
    pub session_id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub lock_owner: Option<String>,
    pub lock_expires_at: Option<DateTime<Utc>>,
    pub preflight_receipt_id: Uuid,
    pub preflight_receipt_checksum: String,
    pub config_receipt_id: Uuid,
    pub config_receipt_checksum: String,
    pub database_receipt_id: Uuid,
    pub database_receipt_checksum: String,
    pub migrate_receipt_id: Uuid,
    pub migrate_receipt_checksum: String,
    pub seed_receipt_id: Uuid,
    pub seed_receipt_checksum: String,
    pub admin_receipt_id: Uuid,
    pub admin_receipt_checksum: String,
    pub verify_receipt_id: Uuid,
    pub verify_receipt_checksum: String,
    pub finalize_receipt_id: Uuid,
    pub finalize_receipt_checksum: String,
    pub deployment_receipts: Vec<crate::InstallRoleDeploymentReceipt>,
    pub next: Option<String>,
}

/// Boundary implemented by a host-specific installer runtime.
#[async_trait::async_trait]
pub trait InstallExecutor: Send + Sync {
    async fn apply(
        &self,
        plan: InstallPlan,
        options: InstallApplyOptions,
    ) -> Result<InstallApplyOutput, InstallExecutionError>;
}

/// Durable installer-session projection independent of a persistence backend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallSessionRecord {
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub lock_owner: Option<String>,
    pub lock_expires_at: Option<DateTime<Utc>>,
}

/// Durable receipt identity returned by a persistence adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallReceiptRecord {
    pub id: Uuid,
    pub input_checksum: String,
}

/// Result of the database-readiness stage, retaining an adapter-private runtime
/// handle for the later schema, seed, admin and verification stages.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallDatabaseReady<R> {
    #[serde(skip)]
    pub runtime: R,
    pub database_name: Option<String>,
    pub created_database: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallSeedOutcome {
    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub tenant_created: bool,
    pub enabled_modules: Vec<String>,
    pub disabled_modules: Vec<String>,
    pub demo_customer_created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallAdminOutcome {
    pub user_id: Uuid,
    pub email: String,
    pub created: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstallVerificationOutcome {
    pub tenant_id: Uuid,
    pub tenant_slug: String,
    pub admin_user_id: Uuid,
    pub enabled_modules: Vec<String>,
}

/// Database readiness boundary. The adapter owns database-driver selection and
/// optional database creation; orchestration remains framework independent.
#[async_trait::async_trait]
pub trait InstallDatabasePort: Send + Sync {
    type Runtime: Send + Sync;

    async fn prepare_database(
        &self,
        plan: &InstallPlan,
        database_url: &str,
        options: &InstallApplyOptions,
    ) -> Result<InstallDatabaseReady<Self::Runtime>, InstallExecutionError>;
}

#[async_trait::async_trait]
pub trait InstallSchemaPort<R>: Send + Sync {
    async fn apply_schema(&self, runtime: &R) -> Result<(), InstallExecutionError>;
}

#[async_trait::async_trait]
pub trait InstallPersistencePort<R>: Send + Sync {
    async fn create_session(
        &self,
        runtime: &R,
        plan: &InstallPlan,
    ) -> Result<InstallSessionRecord, InstallExecutionError>;
    async fn acquire_lock(
        &self,
        runtime: &R,
        session: InstallSessionRecord,
        owner: &str,
        ttl_secs: i64,
    ) -> Result<InstallSessionRecord, InstallExecutionError>;
    async fn record_receipt(
        &self,
        runtime: &R,
        receipt: &InstallReceipt,
    ) -> Result<InstallReceiptRecord, InstallExecutionError>;
    async fn set_state(
        &self,
        runtime: &R,
        session_id: Uuid,
        state: InstallState,
    ) -> Result<InstallSessionRecord, InstallExecutionError>;
    async fn set_tenant_id(
        &self,
        runtime: &R,
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<InstallSessionRecord, InstallExecutionError>;
}

#[async_trait::async_trait]
pub trait InstallSeedPort<R>: Send + Sync {
    async fn apply_seed(
        &self,
        runtime: &R,
        plan: &InstallPlan,
    ) -> Result<InstallSeedOutcome, InstallExecutionError>;
}

#[async_trait::async_trait]
pub trait InstallAdminPort<R>: Send + Sync {
    async fn provision_admin(
        &self,
        runtime: &R,
        plan: &InstallPlan,
        tenant_id: Uuid,
        password: &str,
    ) -> Result<InstallAdminOutcome, InstallExecutionError>;
}

#[async_trait::async_trait]
pub trait InstallVerificationPort<R>: Send + Sync {
    async fn verify_installation(
        &self,
        runtime: &R,
        plan: &InstallPlan,
        tenant_id: Uuid,
    ) -> Result<InstallVerificationOutcome, InstallExecutionError>;
}

/// Executes the canonical installer state machine through host-provided stage
/// ports. HTTP and CLI adapters must invoke this function rather than duplicate
/// stage ordering, state transitions, or receipt construction.
pub async fn execute_install_apply<P>(
    ports: &P,
    plan: InstallPlan,
    options: InstallApplyOptions,
) -> Result<InstallApplyOutput, InstallExecutionError>
where
    P: InstallDatabasePort
        + InstallSchemaPort<P::Runtime>
        + InstallPersistencePort<P::Runtime>
        + InstallSeedPort<P::Runtime>
        + InstallAdminPort<P::Runtime>
        + InstallVerificationPort<P::Runtime>
        + crate::InstallDeploymentPort<P::Runtime>,
{
    let report = crate::evaluate_preflight_with_deployment(
        &plan,
        crate::InstallDeploymentPort::supports_distributed_deployment(ports),
    );
    if !report.passed() {
        return Err(InstallExecutionError::new("installer preflight failed"));
    }
    let database_url = crate::resolve_local_secret_value(&plan.database.url, "database URL")
        .map_err(|error| InstallExecutionError::new(error.to_string()))?;
    let database = ports
        .prepare_database(&plan, &database_url, &options)
        .await?;
    ports.apply_schema(&database.runtime).await?;
    let snapshot = crate::redact_install_plan(&plan);
    let mut session = ports.create_session(&database.runtime, &plan).await?;
    session = ports
        .acquire_lock(
            &database.runtime,
            session,
            &options.lock_owner,
            options.lock_ttl_secs.max(1),
        )
        .await?;

    let preflight = record_success(
        ports,
        &database.runtime,
        &session,
        InstallStep::Preflight,
        &snapshot,
        serde_json::json!({
            "report": report,
            "mode": "apply",
            "note": "preflight receipt recorded after database and schema bootstrap"
        }),
    )
    .await?;
    session = ports
        .set_state(&database.runtime, session.id, InstallState::PreflightPassed)
        .await?;
    let config = record_success(
        ports,
        &database.runtime,
        &session,
        InstallStep::Config,
        &snapshot,
        serde_json::json!({
            "source": "installer",
            "secrets_mode": plan.secrets_mode,
            "redacted": true
        }),
    )
    .await?;
    session = ports
        .set_state(&database.runtime, session.id, InstallState::ConfigPrepared)
        .await?;
    let database_receipt = record_success(
        ports,
        &database.runtime,
        &session,
        InstallStep::Database,
        &snapshot,
        serde_json::json!({
            "database_engine": plan.database.engine,
            "database_name": database.database_name,
            "create_if_missing": plan.database.create_if_missing,
            "created_database": database.created_database,
            "checked": true
        }),
    )
    .await?;
    session = ports
        .set_state(&database.runtime, session.id, InstallState::DatabaseReady)
        .await?;
    let migrate = record_success(
        ports,
        &database.runtime,
        &session,
        InstallStep::Migrate,
        &snapshot,
        serde_json::json!({
            "migrator": "rustok-migrations::Migrator",
            "limit": null,
            "applied": "up_to_latest"
        }),
    )
    .await?;
    session = ports
        .set_state(&database.runtime, session.id, InstallState::SchemaApplied)
        .await?;
    let seed_outcome = ports.apply_seed(&database.runtime, &plan).await?;
    session = ports
        .set_tenant_id(&database.runtime, session.id, seed_outcome.tenant_id)
        .await?;
    let seed = record_success(
        ports,
        &database.runtime,
        &session,
        InstallStep::Seed,
        &snapshot,
        serde_json::json!({
            "seed_profile": plan.seed_profile,
            "tenant_id": seed_outcome.tenant_id,
            "tenant_slug": seed_outcome.tenant_slug,
            "tenant_created": seed_outcome.tenant_created,
            "enabled_modules": seed_outcome.enabled_modules,
            "disabled_modules": seed_outcome.disabled_modules,
            "demo_customer_created": seed_outcome.demo_customer_created
        }),
    )
    .await?;
    session = ports
        .set_state(&database.runtime, session.id, InstallState::SeedApplied)
        .await?;
    let password = crate::resolve_local_secret_value(&plan.admin.password, "admin password")
        .map_err(|error| InstallExecutionError::new(error.to_string()))?;
    let admin_outcome = ports
        .provision_admin(&database.runtime, &plan, seed_outcome.tenant_id, &password)
        .await?;
    let admin = record_success(
        ports,
        &database.runtime,
        &session,
        InstallStep::Admin,
        &snapshot,
        serde_json::json!({
            "tenant_id": seed_outcome.tenant_id,
            "admin_email": admin_outcome.email,
            "admin_user_id": admin_outcome.user_id,
            "admin_created": admin_outcome.created,
            "role": "super_admin"
        }),
    )
    .await?;
    session = ports
        .set_state(
            &database.runtime,
            session.id,
            InstallState::AdminProvisioned,
        )
        .await?;
    let deployment_receipts = if plan.topology.mode == crate::InstallTopologyMode::Distributed {
        let output = crate::execute_distributed_role_deployments(
            ports,
            &database.runtime,
            &plan,
            session,
            seed_outcome.tenant_id,
        )
        .await?;
        session = output.session;
        output.receipts
    } else {
        Vec::new()
    };
    let verify_outcome = ports
        .verify_installation(&database.runtime, &plan, seed_outcome.tenant_id)
        .await?;
    let verify = record_success(
        ports,
        &database.runtime,
        &session,
        InstallStep::Verify,
        &snapshot,
        serde_json::json!({
            "tenant_id": verify_outcome.tenant_id,
            "tenant_slug": verify_outcome.tenant_slug,
            "admin_user_id": verify_outcome.admin_user_id,
            "enabled_modules": verify_outcome.enabled_modules
        }),
    )
    .await?;
    session = ports
        .set_state(&database.runtime, session.id, InstallState::Verified)
        .await?;
    let finalize = record_success(
        ports,
        &database.runtime,
        &session,
        InstallStep::Finalize,
        &snapshot,
        serde_json::json!({
            "completed": true,
            "tenant_id": verify_outcome.tenant_id,
            "tenant_slug": verify_outcome.tenant_slug
        }),
    )
    .await?;
    session = ports
        .set_state(&database.runtime, session.id, InstallState::Completed)
        .await?;

    Ok(InstallApplyOutput {
        status: "completed".to_string(),
        session_id: session.id,
        tenant_id: session.tenant_id,
        lock_owner: session.lock_owner,
        lock_expires_at: session.lock_expires_at,
        preflight_receipt_id: preflight.id,
        preflight_receipt_checksum: preflight.input_checksum,
        config_receipt_id: config.id,
        config_receipt_checksum: config.input_checksum,
        database_receipt_id: database_receipt.id,
        database_receipt_checksum: database_receipt.input_checksum,
        migrate_receipt_id: migrate.id,
        migrate_receipt_checksum: migrate.input_checksum,
        seed_receipt_id: seed.id,
        seed_receipt_checksum: seed.input_checksum,
        admin_receipt_id: admin.id,
        admin_receipt_checksum: admin.input_checksum,
        verify_receipt_id: verify.id,
        verify_receipt_checksum: verify.input_checksum,
        finalize_receipt_id: finalize.id,
        finalize_receipt_checksum: finalize.input_checksum,
        deployment_receipts,
        next: None,
    })
}

async fn record_success<P>(
    ports: &P,
    runtime: &P::Runtime,
    session: &InstallSessionRecord,
    step: InstallStep,
    snapshot: &serde_json::Value,
    diagnostics: serde_json::Value,
) -> Result<InstallReceiptRecord, InstallExecutionError>
where
    P: InstallDatabasePort + InstallPersistencePort<P::Runtime>,
{
    let receipt = InstallReceipt::success(session.id.to_string(), step, snapshot, diagnostics)
        .map_err(|error| InstallExecutionError::new(error.to_string()))?;
    ports.record_receipt(runtime, &receipt).await
}

#[derive(Debug, Error)]
#[error("install execution failed: {message}")]
pub struct InstallExecutionError {
    message: String,
}

impl InstallExecutionError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}
