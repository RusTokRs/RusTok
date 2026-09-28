use chrono::Utc;
use rustok_installer::{InstallApplyOutput, InstallPlan, InstallReceipt, InstallState, redact_install_plan};
use sea_orm::{
    AccessMode, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection,
    DatabaseTransaction, EntityTrait, IsolationLevel, QueryFilter, QueryOrder, TransactionTrait,
};
use uuid::Uuid;

use crate::entities::{install_http_job, install_session, install_step_receipt};

const MAX_HTTP_INSTALL_JOB_OUTPUT_BYTES: usize = 256 * 1024;
const HTTP_INSTALL_JOB_RUNNING_STATUS: &str = "running";
const HTTP_INSTALL_JOB_SUCCEEDED_STATUS: &str = "succeeded";
const HTTP_INSTALL_JOB_FAILED_STATUS: &str = "failed";

#[derive(Clone)]
pub struct InstallerPersistenceService {
    db: DatabaseConnection,
}

impl InstallerPersistenceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn create_http_job(
        &self,
        job_id: Uuid,
        submitted_at: chrono::DateTime<Utc>,
    ) -> Result<install_http_job::Model, sea_orm::DbErr> {
        install_http_job::ActiveModel {
            id: Set(job_id),
            status: Set(HTTP_INSTALL_JOB_RUNNING_STATUS.to_string()),
            submitted_at: Set(submitted_at),
            started_at: Set(submitted_at),
            finished_at: Set(None),
            session_id: Set(None),
            tenant_id: Set(None),
            output: Set(None),
            error_message: Set(None),
            updated_at: Set(submitted_at),
        }
        .insert(&self.db)
        .await
    }

    pub async fn get_http_job(
        &self,
        job_id: Uuid,
    ) -> Result<Option<install_http_job::Model>, sea_orm::DbErr> {
        install_http_job::Entity::find_by_id(job_id)
            .one(&self.db)
            .await
    }

    pub async fn finish_http_job_succeeded(
        &self,
        job_id: Uuid,
        session_id: Uuid,
        tenant_id: Uuid,
        output: &InstallApplyOutput,
    ) -> Result<install_http_job::Model, sea_orm::DbErr> {
        let now = Utc::now();
        let serialized = serde_json::to_value(output).map_err(|error| {
            sea_orm::DbErr::Custom(format!("failed to serialize installer job output: {error}"))
        })?;
        let output = match serde_json::to_vec(&serialized) {
            Ok(bytes) if bytes.len() <= MAX_HTTP_INSTALL_JOB_OUTPUT_BYTES => Some(serialized),
            Ok(_) => None
            Err(error) => {
                return Err(sea_orm::DbErr::Custom(format!(
                    "failed to size installer job output: {error}"
                )));
            }
        };

        let result = install_http_job::Entity::update_many()
            .filter(install_http_job::Column::Id.eq(job_id))
            .filter(install_http_job::Column::Status.eq(HTTP_INSTALL_JOB_RUNNING_STATUS))
            .set(install_http_job::ActiveModel {
                status: Set(HTTP_INSTALL_JOB_SUCCEEDED_STATUS.to_string()),
                finished_at: Set(Some(now)),
                session_id: Set(Some(session_id)),
                tenant_id: Set(Some(tenant_id)),
                output: Set(output),
                error_message: Set(None),
                updated_at: Set(now),
                ..Default::default()
            })
            .exec(&self.db)
            .await?;

        ensure_http_job_transition_applied(job_id, result.rows_affected)?;
        self.get_http_job(job_id)
            .await?
            .ok_or_else(|| sea_orm::DbErr::RecordNotFound(format!("installer HTTP job {job_id}")))
    }

    pub async fn finish_http_job_failed(
        &self,
        job_id: Uuid,
        error_message: &str,
    ) -> Result<install_http_job::Model, sea_orm::DbErr> {
        let message = bounded_http_job_error_message(error_message);
        let now = Utc::now();
        let result = install_http_job::Entity::update_many()
            .filter(install_http_job::Column::Id.eq(job_id))
            .filter(install_http_job::Column::Status.eq(HTTP_INSTALL_JOB_RUNNING_STATUS))
            .set(install_http_job::ActiveModel {
                status: Set(HTTP_INSTALL_JOB_FAILED_STATUS.to_string()),
                finished_at: Set(Some(now)),
                error_message: Set(Some(message)),
                output: Set(None),
                updated_at: Set(now),
                ..Default::default()
            })
            .exec(&self.db)
            .await?;

        ensure_http_job_transition_applied(job_id, result.rows_affected)?;
        self.get_http_job(job_id)
            .await?
            .ok_or_else(|| sea_orm::DbErr::RecordNotFound(format!("installer HTTP job {job_id}")))
    }

    pub async fn create_session(
        &self,
        plan: &InstallPlan,
        tenant_id: Option<Uuid>,
        created_by: Option<Uuid>,
    ) -> Result<install_session::Model, sea_orm::DbErr> {
        let now = Utc::now();
        install_session::ActiveModel {
            id: Set(rustok_core::generate_id()),
            tenant_id: Set(tenant_id),
            status: Set(install_state_value(InstallState::Draft).to_string()),
            profile: Set(serde_name(plan.profile)),
            environment: Set(serde_name(plan.environment)),
            database_engine: Set(serde_name(plan.database.engine)),
            seed_profile: Set(serde_name(plan.seed_profile)),
            plan_snapshot: Set(redact_install_plan(plan)),
            lock_owner: Set(None),
            lock_expires_at: Set(None),
            error_message: Set(None),
            created_by: Set(created_by),
            created_at: Set(now),
            updated_at: Set(now),
            completed_at: Set(None),
        }
        .insert(&self.db)
        .await
    }

    pub async fn record_receipt(
        &self,
        receipt: &InstallReceipt,
    ) -> Result<install_step_receipt::Model, sea_orm::DbErr> {
        install_step_receipt::ActiveModel {
            id: Set(rustok_core::generate_id()),
            session_id: Set(parse_session_uuid(&receipt.session_id)?),
            step: Set(serde_name(receipt.step)),
            outcome: Set(serde_name(receipt.outcome)),
            input_checksum: Set(receipt.input_checksum.clone()),
            diagnostics: Set(receipt.diagnostics.clone()),
            installer_version: Set(receipt.installer_version.clone()),
            created_at: Set(receipt.created_at),
        }
        .insert(&self.db)
        .await
    }

    pub async fn acquire_lock(
        &self,
        session: install_session::Model,
        owner: &str,
        ttl: chrono::Duration,
    ) -> Result<install_session::Model, sea_orm::DbErr> {
        let owner = owner.trim();
        if owner.is_empty() {
            return Err(sea_orm::DbErr::Custom(
                "installer lock owner must not be empty".to_string(),
            ));
        }
        let ttl = ttl.max(chrono::Duration::seconds(1));
        let session_id = session.id;

        let txn = match self.db.get_database_backend() {
            sea_orm::DbBackend::Postgres => {
                self.db
                    .begin_with_config(
                        Some(IsolationLevel::Serializable),
                        Some(AccessMode::ReadWrite),
                    )
                    .await?
            }
            _ => self.db.begin().await?,
        };

        let result = acquire_lock_in_transaction(&txn, session, owner, ttl).await?;
        txn.commit().await?;

        result.ok_or_else(|| {
            sea_orm::DbErr::RecordNotFound(format!("install session {session_id} not found"))
        })
    }

    pub async fn set_state(
        &self,
        session_id: Uuid,
        state: InstallState,
    ) -> Result<install_session::Model, sea_orm::DbErr> {
        let Some(session) = self.get_session(session_id).await? else {
            return Err(sea_orm::DbErr::RecordNotFound(format!(
                "install session {session_id} not found"
            )));
        };
        let now = Utc::now();
        let mut active: install_session::ActiveModel = session.into();
        active.status = Set(install_state_value(state).to_string());
        active.updated_at = Set(now);
        if state == InstallState::Completed {
            active.completed_at = Set(Some(now));
        }
        active.update(&self.db).await
    }

    pub async fn set_tenant_id(
        &self,
        session_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<install_session::Model, sea_orm::DbErr> {
        let Some(session) = self.get_session(session_id).await? else {
            return Err(sea_orm::DbErr::RecordNotFound(format!(
                "install session {session_id} not found"
            )));
        };
        let now = Utc::now();
        let mut active: install_session::ActiveModel = session.into();
        active.tenant_id = Set(Some(tenant_id));
        active.updated_at = Set(now);
        active.update(&self.db).await
    }

    pub async fn get_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<install_session::Model>, sea_orm::DbErr> {
        install_session::Entity::find_by_id(session_id)
            .one(&self.db)
            .await
    }

    pub async fn latest_session(&self) -> Result<Option<install_session::Model>, sea_orm::DbErr> {
        install_session::Entity::find()
            .order_by_desc(install_session::Column::CreatedAt)
            .one(&self.db)
            .await
    }

    pub async fn list_receipts(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<install_step_receipt::Model>, sea_orm::DbErr> {
        install_step_receipt::Entity::find()
            .filter(install_step_receipt::Column::SessionId.eq(session_id))
            .order_by_asc(install_step_receipt::Column::CreatedAt)
            .all(&self.db)
            .await
    }
}

fn ensure_http_job_transition_applied(
    job_id: Uuid,
    rows_affected: u64,
) -> Result<(), sea_orm::DbErr> {
    if rows_affected == 1 {
        return Ok(());
    }

    Err(sea_orm::DbErr::Custom(format!(
        "installer HTTP job {job_id} is not in running state"
    )))
}

fn bounded_http_job_error_message(_error_message: &str) -> String {
    "installer apply failed; inspect durable installer receipts for recovery details".to_string()
}

async fn acquire_lock_in_transaction(
    txn: &DatabaseTransaction,
    session: install_session::Model,
    owner: &str,
    ttl: chrono::Duration,
) -> Result<Option<install_session::Model>, sea_orm::DbErr> {
    let now = Utc::now();
    if let Some(existing) = install_session::Entity::find()
        .filter(install_session::Column::Id.ne(session.id))
        .filter(install_session::Column::LockExpiresAt.gt(now))
        .filter(install_session::Column::Status.is_not_in(final_state_values()))
        .order_by_desc(install_session::Column::LockExpiresAt)
        .one(txn)
        .await?
    {
        return Err(sea_orm::DbErr::Custom(format!(
            "installer lock is already held by session {}",
            existing.id
        )));
    }

    let mut active: install_session::ActiveModel = session.into();
    active.lock_owner = Set(Some(owner.to_string()));
    active.lock_expires_at = Set(Some(now + ttl));
    active.updated_at = Set(now);
    active.update(txn).await.map(Some)
}

fn parse_session_uuid(value: &str) -> Result<Uuid, sea_orm::DbErr> {
    Uuid::parse_str(value)
        .map_err(|error| sea_orm::DbErr::Custom(format!("invalid install session id: {error}")))
}

fn install_state_value(state: InstallState) -> &'static str {
    match state {
        InstallState::Draft => "draft",
        InstallState::PreflightPassed => "preflight_passed",
        InstallState::ConfigPrepared => "config_prepared",
        InstallState::DatabaseReady => "database_ready",
        InstallState::SchemaApplied => "schema_applied",
        InstallState::SeedApplied => "seed_applied",
        InstallState::AdminProvisioned => "admin_provisioned",
        InstallState::Deploying => "deploying",
        InstallState::Verified => "verified",
        InstallState::Completed => "completed",
        InstallState::Failed => "failed",
        InstallState::FreshInstallCleaned => "fresh_install_cleaned",
        InstallState::RecoveryRequired => "recovery_required",
    }
}

fn final_state_values() -> Vec<&'static str> {
    vec![
        install_state_value(InstallState::Completed),
        install_state_value(InstallState::Failed),
        install_state_value(InstallState::FreshInstallCleaned),
        install_state_value(InstallState::RecoveryRequired),
    ]
}

fn serde_name<T: serde::Serialize>(value: T) -> String {
    let json =
        serde_json::to_value(value).expect("installer enum serialization must be infallible");
    json.as_str()
        .expect("installer enum serialization must produce a string")
        .to_string()
}
