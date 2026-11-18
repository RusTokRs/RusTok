use chrono::Utc;
use rustok_installer::{
    InstallApplyOutput, InstallPlan, InstallReceipt, InstallSessionRecord, InstallState,
    InstallStep, ReceiptOutcome, is_valid_install_lock_ttl_secs, redact_install_plan,
    MAX_INSTALL_LOCK_TTL_SECS,
};
use sea_orm::{
    AccessMode, ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection,
    ConnectionTrait, DatabaseTransaction, EntityTrait, IsolationLevel, QueryFilter, QueryOrder,
    QuerySelect, TransactionTrait,
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
        session_id: Uuid,
        submitted_at: chrono::DateTime<Utc>,
    ) -> Result<install_http_job::Model, sea_orm::DbErr> {
        if job_id.is_nil() || session_id.is_nil() {
            return Err(sea_orm::DbErr::Custom(
                "installer job and session IDs must not be nil".to_string(),
            ));
        }
        install_http_job::ActiveModel {
            id: Set(job_id),
            status: Set(HTTP_INSTALL_JOB_RUNNING_STATUS.to_string()),
            submitted_at: Set(submitted_at),
            started_at: Set(submitted_at),
            finished_at: Set(None),
            session_id: Set(Some(session_id)),
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

    pub async fn has_running_http_job(&self) -> Result<bool, sea_orm::DbErr> {
        install_http_job::Entity::find()
            .filter(install_http_job::Column::Status.eq(HTTP_INSTALL_JOB_RUNNING_STATUS))
            .one(&self.db)
            .await
            .map(|job| job.is_some())
    }

    pub async fn finish_http_job_succeeded(
        &self,
        job_id: Uuid,
        session_id: Uuid,
        tenant_id: Option<Uuid>,
        output: &InstallApplyOutput,
    ) -> Result<install_http_job::Model, sea_orm::DbErr> {
        let now = Utc::now();
        let serialized = serde_json::to_value(output).map_err(|error| {
            sea_orm::DbErr::Custom(format!("failed to serialize installer job output: {error}"))
        })?;
        let output = match serde_json::to_vec(&serialized) {
            Ok(bytes) if bytes.len() <= MAX_HTTP_INSTALL_JOB_OUTPUT_BYTES => Some(serialized),
            Ok(_) => None,
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
                tenant_id: Set(tenant_id),
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
        self.create_session_with_id(rustok_core::generate_id(), plan, tenant_id, created_by)
            .await
    }

    pub async fn create_session_with_id(
        &self,
        session_id: Uuid,
        plan: &InstallPlan,
        tenant_id: Option<Uuid>,
        created_by: Option<Uuid>,
    ) -> Result<install_session::Model, sea_orm::DbErr> {
        if session_id.is_nil() {
            return Err(sea_orm::DbErr::Custom(
                "installer session ID must not be nil".to_string(),
            ));
        }
        let now = Utc::now();
        install_session::ActiveModel {
            id: Set(session_id),
            tenant_id: Set(tenant_id),
            status: Set(install_state_value(InstallState::Draft).to_string()),
            profile: Set(serde_name(plan.profile)),
            environment: Set(serde_name(plan.environment)),
            database_engine: Set(serde_name(plan.database.engine)),
            seed_profile: Set(serde_name(plan.seed_profile)),
            plan_snapshot: Set(redact_install_plan(plan)),
            attempt_generation: Set(0),
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
        session: &InstallSessionRecord,
        receipt: &InstallReceipt,
    ) -> Result<install_step_receipt::Model, sea_orm::DbErr> {
        let owner = require_attempt_fence(session)?;
        let receipt_session_id = parse_session_uuid(&receipt.session_id)?;
        if receipt_session_id != session.id {
            return Err(sea_orm::DbErr::Custom(
                "installer receipt session does not match its attempt fence".to_string(),
            ));
        }
        let txn = self.db.begin().await?;
        let current = lock_session_for_attempt(&txn, session).await?;
        let current_state = InstallState::from_str(&current.status).ok_or_else(|| {
            sea_orm::DbErr::Custom(format!(
                "installer session {} has an unknown persisted state",
                session.id
            ))
        })?;
        let is_recovery_rollback = current_state == InstallState::RecoveryRequired
            && receipt.step == InstallStep::Rollback;
        if matches!(
            current_state,
            InstallState::Completed | InstallState::Failed | InstallState::FreshInstallCleaned
        ) || (current_state == InstallState::RecoveryRequired && !is_recovery_rollback)
        {
            return Err(sea_orm::DbErr::Custom(format!(
                "installer session {} cannot accept this receipt in state {current_state:?}",
                session.id
            )));
        }

        let now = Utc::now();
        let touch = install_session::Entity::update_many()
            .col_expr(
                install_session::Column::UpdatedAt,
                sea_orm::sea_query::Expr::value(now),
            )
            .filter(install_session::Column::Id.eq(session.id))
            .filter(install_session::Column::LockOwner.eq(owner))
            .filter(
                install_session::Column::AttemptGeneration.eq(session.attempt_generation),
            )
            .filter(install_session::Column::Status.eq(current.status))
            .exec(&txn)
            .await?;
        ensure_attempt_fence_applied(session, touch.rows_affected)?;

        let model = install_step_receipt::ActiveModel {
            id: Set(rustok_core::generate_id()),
            session_id: Set(session.id),
            attempt_generation: Set(session.attempt_generation),
            step: Set(serde_name(receipt.step)),
            outcome: Set(serde_name(receipt.outcome)),
            input_checksum: Set(receipt.input_checksum.clone()),
            diagnostics: Set(receipt.diagnostics.clone()),
            installer_version: Set(receipt.installer_version.clone()),
            created_at: Set(receipt.created_at),
        }
        .insert(&txn)
        .await?;

        txn.commit().await?;
        Ok(model)
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
        if owner.len() > 128 {
            return Err(sea_orm::DbErr::Custom(
                "installer lock owner must fit the 128-byte storage limit".to_string(),
            ));
        }
        let ttl_secs = ttl.num_seconds();
        if !is_valid_install_lock_ttl_secs(ttl_secs) {
            return Err(sea_orm::DbErr::Custom(format!(
                "installer lock TTL must be between 1 and {MAX_INSTALL_LOCK_TTL_SECS} seconds"
            )));
        }
        let ttl = chrono::Duration::seconds(ttl_secs);
        let session_id = session.id;

        let txn = match self.db.get_database_backend() {
            sea_orm::DbBackend::Postgres | sea_orm::DbBackend::MySql => {
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
        session: &InstallSessionRecord,
        state: InstallState,
    ) -> Result<install_session::Model, sea_orm::DbErr> {
        let owner = require_attempt_fence(session)?;
        let now = Utc::now();
        let txn = self.db.begin().await?;
        let current = lock_session_for_attempt(&txn, session).await?;
        let current_state = InstallState::from_str(&current.status).ok_or_else(|| {
            sea_orm::DbErr::Custom(format!(
                "installer session {} has an unknown persisted state",
                session.id
            ))
        })?;
        if !current_state.can_transition_to(state) {
            return Err(sea_orm::DbErr::Custom(format!(
                "invalid installer state transition from {current_state:?} to {state:?}"
            )));
        }

        for step in required_receipt_steps_for_state(state) {
            let receipt = install_step_receipt::Entity::find()
                .filter(install_step_receipt::Column::SessionId.eq(session.id))
                .filter(
                    install_step_receipt::Column::AttemptGeneration
                        .eq(session.attempt_generation),
                )
                .filter(install_step_receipt::Column::Step.eq(serde_name(*step)))
                .filter(
                    install_step_receipt::Column::Outcome.eq(serde_name(ReceiptOutcome::Success)),
                )
                .one(&txn)
                .await?;
            if receipt.is_none() {
                return Err(sea_orm::DbErr::Custom(format!(
                    "installer state {state:?} requires a successful {step:?} receipt for attempt generation {}",
                    session.attempt_generation
                )));
            }
        }

        let mut active = install_session::ActiveModel {
            status: Set(install_state_value(state).to_string()),
            updated_at: Set(now),
            ..Default::default()
        };
        if state == InstallState::Completed {
            active.completed_at = Set(Some(now));
        }

        let result = install_session::Entity::update_many()
            .set(active)
            .filter(install_session::Column::Id.eq(session.id))
            .filter(install_session::Column::LockOwner.eq(owner))
            .filter(
                install_session::Column::AttemptGeneration.eq(session.attempt_generation),
            )
            .filter(install_session::Column::Status.eq(current.status))
            .exec(&txn)
            .await?;
        ensure_attempt_fence_applied(session, result.rows_affected)?;
        let updated = install_session::Entity::find_by_id(session.id)
            .one(&txn)
            .await?
            .ok_or_else(|| {
                sea_orm::DbErr::RecordNotFound(format!(
                    "install session {} not found",
                    session.id
                ))
            })?;
        txn.commit().await?;
        Ok(updated)
    }

    pub async fn set_tenant_id(
        &self,
        session: &InstallSessionRecord,
        tenant_id: Uuid,
    ) -> Result<install_session::Model, sea_orm::DbErr> {
        let owner = require_attempt_fence(session)?;
        let now = Utc::now();
        let txn = self.db.begin().await?;
        let current = lock_session_for_attempt(&txn, session).await?;
        let current_state = InstallState::from_str(&current.status).ok_or_else(|| {
            sea_orm::DbErr::Custom(format!(
                "installer session {} has an unknown persisted state",
                session.id
            ))
        })?;
        if current_state != InstallState::SchemaApplied {
            return Err(sea_orm::DbErr::Custom(format!(
                "installer tenant assignment requires schema_applied state, found {current_state:?}"
            )));
        }
        if current.tenant_id.is_some_and(|existing| existing != tenant_id) {
            return Err(sea_orm::DbErr::Custom(format!(
                "installer session {} already belongs to a different tenant",
                session.id
            )));
        }

        let result = install_session::Entity::update_many()
            .set(install_session::ActiveModel {
                tenant_id: Set(Some(tenant_id)),
                updated_at: Set(now),
                ..Default::default()
            })
            .filter(install_session::Column::Id.eq(session.id))
            .filter(install_session::Column::LockOwner.eq(owner))
            .filter(
                install_session::Column::AttemptGeneration.eq(session.attempt_generation),
            )
            .filter(install_session::Column::Status.eq(current.status))
            .filter(
                sea_orm::Condition::any()
                    .add(install_session::Column::TenantId.is_null())
                    .add(install_session::Column::TenantId.eq(tenant_id)),
            )
            .exec(&txn)
            .await?;
        ensure_attempt_fence_applied(session, result.rows_affected)?;
        let updated = install_session::Entity::find_by_id(session.id)
            .one(&txn)
            .await?
            .ok_or_else(|| {
                sea_orm::DbErr::RecordNotFound(format!(
                    "install session {} not found",
                    session.id
                ))
            })?;
        txn.commit().await?;
        Ok(updated)
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

    pub async fn has_completed_session(&self) -> Result<bool, sea_orm::DbErr> {
        install_session::Entity::find()
            .filter(
                install_session::Column::Status.eq(install_state_value(InstallState::Completed)),
            )
            .one(&self.db)
            .await
            .map(|session| session.is_some())
    }

    pub async fn list_receipts(
        &self,
        session_id: Uuid,
    ) -> Result<Vec<InstallStepReceiptItem>, sea_orm::DbErr> {
        let receipts = install_step_receipt::Entity::find()
            .filter(install_step_receipt::Column::SessionId.eq(session_id))
            .order_by_asc(install_step_receipt::Column::CreatedAt)
            .all(&self.db)
            .await?;
        Ok(receipts.into_iter().map(Into::into).collect())
    }
}

/// Typed installer step receipt DTO decoupled from persistence entities.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct InstallStepReceiptItem {
    pub id: Uuid,
    pub session_id: Uuid,
    pub attempt_generation: i64,
    pub step: String,
    pub outcome: String,
    pub input_checksum: String,
    pub diagnostics: serde_json::Value,
    pub installer_version: String,
    pub created_at: chrono::DateTime<Utc>,
}

impl From<install_step_receipt::Model> for InstallStepReceiptItem {
    fn from(model: install_step_receipt::Model) -> Self {
        Self {
            id: model.id,
            session_id: model.session_id,
            attempt_generation: model.attempt_generation,
            step: model.step,
            outcome: model.outcome,
            input_checksum: model.input_checksum,
            diagnostics: model.diagnostics,
            installer_version: model.installer_version,
            created_at: model.created_at,
        }
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
    let lock_expires_at = now.checked_add_signed(ttl).ok_or_else(|| {
        sea_orm::DbErr::Custom(
            "installer lock TTL exceeds the supported timestamp range".to_string(),
        )
    })?;

    if session.attempt_generation < 0 {
        return Err(sea_orm::DbErr::Custom(
            "installer attempt generation must not be negative".to_string(),
        ));
    }
    let next_generation = session.attempt_generation.checked_add(1).ok_or_else(|| {
        sea_orm::DbErr::Custom("installer attempt generation is exhausted".to_string())
    })?;

    // Claim this session exactly once. The generation is advanced in the same
    // compare-and-set as the owner, so stale workers cannot reuse an old fence.
    let claim = install_session::Entity::update_many()
        .col_expr(
            install_session::Column::LockOwner,
            sea_orm::sea_query::Expr::value(Some(owner.to_string())),
        )
        .col_expr(
            install_session::Column::LockExpiresAt,
            sea_orm::sea_query::Expr::value(Some(lock_expires_at)),
        )
        .col_expr(
            install_session::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .col_expr(
            install_session::Column::AttemptGeneration,
            sea_orm::sea_query::Expr::value(next_generation),
        )
        .filter(install_session::Column::Id.eq(session.id))
        .filter(
            install_session::Column::AttemptGeneration.eq(session.attempt_generation),
        )
        .filter(install_session::Column::LockOwner.is_null())
        .filter(install_session::Column::Status.eq(install_state_value(InstallState::Draft)))
        .exec(txn)
        .await?;
    if claim.rows_affected != 1 {
        if install_session::Entity::find_by_id(session.id)
            .one(txn)
            .await?
            .is_none()
        {
            return Ok(None);
        }
        return Err(sea_orm::DbErr::Custom(format!(
            "installer session {} already has an execution attempt or is finalized",
            session.id
        )));
    }

    // An expired or manually cleared lease is not proof that its worker stopped.
    // Any non-draft session remains a global fence until its effects are reconciled;
    // a draft with no owner is the only state eligible for a fresh claim.
    if let Some(existing) = install_session::Entity::find()
        .filter(install_session::Column::Id.ne(session.id))
        .filter(
            sea_orm::Condition::any()
                .add(install_session::Column::LockOwner.is_not_null())
                .add(
                    install_session::Column::Status
                        .ne(install_state_value(InstallState::Draft)),
                ),
        )
        .filter(install_session::Column::Status.is_not_in(admission_released_state_values()))
        .order_by_desc(install_session::Column::LockExpiresAt)
        .one(txn)
        .await?
    {
        return Err(sea_orm::DbErr::Custom(format!(
            "installer session {} has an unfinished execution; reconcile it before starting another apply",
            existing.id
        )));
    }

    install_session::Entity::find_by_id(session.id)
        .one(txn)
        .await
}

fn parse_session_uuid(value: &str) -> Result<Uuid, sea_orm::DbErr> {
    Uuid::parse_str(value)
        .map_err(|error| sea_orm::DbErr::Custom(format!("invalid install session id: {error}")))
}

fn required_receipt_steps_for_state(state: InstallState) -> &'static [InstallStep] {
    match state {
        InstallState::PreflightPassed => &[InstallStep::Preflight],
        InstallState::ConfigPrepared => &[InstallStep::Config],
        InstallState::DatabaseReady => &[InstallStep::Database],
        InstallState::SchemaApplied => &[InstallStep::Migrate],
        InstallState::SeedApplied => &[InstallStep::Seed],
        InstallState::AdminProvisioned => &[InstallStep::Admin],
        InstallState::Verified => &[InstallStep::Deploy, InstallStep::Verify],
        InstallState::Completed => &[InstallStep::Finalize],
        InstallState::Failed | InstallState::FreshInstallCleaned => &[InstallStep::Rollback],
        InstallState::Draft | InstallState::Deploying | InstallState::RecoveryRequired => &[],
    }
}

fn install_state_value(state: InstallState) -> &'static str {
    state.as_str()
}

fn require_attempt_fence(session: &InstallSessionRecord) -> Result<&str, sea_orm::DbErr> {
    if session.attempt_generation <= 0 {
        return Err(sea_orm::DbErr::Custom(
            "installer mutation requires a positive attempt generation".to_string(),
        ));
    }
    session
        .lock_owner
        .as_deref()
        .filter(|owner| !owner.trim().is_empty())
        .ok_or_else(|| {
            sea_orm::DbErr::Custom(
                "installer mutation requires the current attempt owner".to_string(),
            )
        })
}

fn ensure_attempt_fence_applied(
    session: &InstallSessionRecord,
    rows_affected: u64,
) -> Result<(), sea_orm::DbErr> {
    if rows_affected == 1 {
        return Ok(());
    }

    Err(sea_orm::DbErr::Custom(format!(
        "installer attempt fence rejected mutation for session {} at generation {}",
        session.id, session.attempt_generation
    )))
}

async fn lock_session_for_attempt(
    txn: &DatabaseTransaction,
    session: &InstallSessionRecord,
) -> Result<install_session::Model, sea_orm::DbErr> {
    let owner = require_attempt_fence(session)?;
    let current = match txn.get_database_backend() {
        sea_orm::DbBackend::Postgres | sea_orm::DbBackend::MySql => {
            install_session::Entity::find_by_id(session.id)
                .filter(install_session::Column::LockOwner.eq(owner))
                .filter(
                    install_session::Column::AttemptGeneration.eq(session.attempt_generation),
                )
                .lock_exclusive()
                .one(txn)
                .await?
        }
        sea_orm::DbBackend::Sqlite => {
            // Reserve SQLite's writer before reading the session. This avoids
            // a deferred read-to-write upgrade race while preserving the same
            // owner+generation predicate used by the server databases.
            let fence = install_session::Entity::update_many()
                .col_expr(
                    install_session::Column::UpdatedAt,
                    sea_orm::sea_query::Expr::col(install_session::Column::UpdatedAt),
                )
                .filter(install_session::Column::Id.eq(session.id))
                .filter(install_session::Column::LockOwner.eq(owner))
                .filter(
                    install_session::Column::AttemptGeneration.eq(session.attempt_generation),
                )
                .exec(txn)
                .await?;
            ensure_attempt_fence_applied(session, fence.rows_affected)?;
            install_session::Entity::find_by_id(session.id)
                .one(txn)
                .await?
        }
        backend => {
            return Err(sea_orm::DbErr::Custom(format!(
                "installer attempt fencing is unsupported for database backend {backend:?}"
            )));
        }
    };

    current.ok_or_else(|| {
        sea_orm::DbErr::Custom(format!(
            "installer attempt fence rejected mutation for session {} at generation {}",
            session.id, session.attempt_generation
        ))
    })
}

fn admission_released_state_values() -> Vec<&'static str> {
    // RecoveryRequired is terminal for the executor, but not safe for global
    // admission: an operator must reconcile its ambiguous external effects.
    vec![
        install_state_value(InstallState::Completed),
        install_state_value(InstallState::Failed),
        install_state_value(InstallState::FreshInstallCleaned),
    ]
}

fn serde_name<T: serde::Serialize>(value: T) -> String {
    let json =
        serde_json::to_value(value).expect("installer enum serialization must be infallible");
    json.as_str()
        .expect("installer enum serialization must produce a string")
        .to_string()
}

#[cfg(test)]
mod tests {
    use chrono::Utc;
    use rustok_installer::{
        InstallApplyOutput, InstallComposition, InstallDistributionBinding,
        InstallDistributionDeployment, InstallDistributionDeploymentReceipt,
    };
    use sea_orm::{ConnectionTrait, Database};

    use super::*;

    async fn setup_http_jobs_table() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        db.execute_unprepared(
            r#"
            CREATE TABLE install_http_jobs (
                id TEXT PRIMARY KEY NOT NULL,
                status TEXT NOT NULL,
                submitted_at TEXT NOT NULL,
                started_at TEXT NOT NULL,
                finished_at TEXT NULL,
                session_id TEXT NULL,
                tenant_id TEXT NULL,
                output TEXT NULL,
                error_message TEXT NULL,
                updated_at TEXT NOT NULL
            )
            "#,
        )
        .await
        .expect("install_http_jobs table");
        db
    }

    fn sample_output(session_id: Uuid, tenant_id: Uuid) -> InstallApplyOutput {
        InstallApplyOutput {
            status: "completed".to_string(),
            session_id,
            tenant_id: Some(tenant_id),
            lock_owner: Some("http".to_string()),
            lock_expires_at: None,
            preflight_receipt_id: Uuid::new_v4(),
            preflight_receipt_checksum: "a".to_string(),
            config_receipt_id: Uuid::new_v4(),
            config_receipt_checksum: "b".to_string(),
            database_receipt_id: Uuid::new_v4(),
            database_receipt_checksum: "c".to_string(),
            migrate_receipt_id: Uuid::new_v4(),
            migrate_receipt_checksum: "d".to_string(),
            seed_receipt_id: Uuid::new_v4(),
            seed_receipt_checksum: "e".to_string(),
            admin_receipt_id: Uuid::new_v4(),
            admin_receipt_checksum: "f".to_string(),
            verify_receipt_id: Uuid::new_v4(),
            verify_receipt_checksum: "g".to_string(),
            finalize_receipt_id: Uuid::new_v4(),
            finalize_receipt_checksum: "h".to_string(),
            deployment_receipt: InstallDistributionDeploymentReceipt {
                deployment: InstallDistributionDeployment {
                    composition: InstallComposition {
                        revision: "test-revision".to_string(),
                        hash: "test-hash".to_string(),
                    },
                    distribution: InstallDistributionBinding {
                        preparation_id: Uuid::new_v4(),
                        distribution_release_id: Uuid::new_v4(),
                        bundle_reference: "test@sha256:test".to_string(),
                        bundle_root_digest: "sha256:test".to_string(),
                        role_set_digest: "sha256:test".to_string(),
                        roles: Vec::new(),
                        bootstrap_receipt: None,
                    },
                    rollout_id: Uuid::new_v4(),
                    deployment_reference: "test-deployment".to_string(),
                    observations: Vec::new(),
                },
                receipt_id: Uuid::new_v4(),
                receipt_checksum: "i".to_string(),
            },
            next: Some("done".to_string()),
        }
    }

    #[tokio::test]
    async fn durable_http_job_is_visible_across_persistence_service_instances() {
        let db = setup_http_jobs_table().await;
        let job_id = Uuid::new_v4();
        let submitted_at = Utc::now();

        let writer = InstallerPersistenceService::new(db.clone());
        writer
            .create_http_job(job_id, job_id, submitted_at)
            .await
            .expect("job should be created");

        let reader = InstallerPersistenceService::new(db.clone());
        let stored = reader
            .get_http_job(job_id)
            .await
            .expect("job should be readable")
            .expect("job should exist");

        assert_eq!(stored.id, job_id);
        assert_eq!(stored.status, "running");
        assert_eq!(stored.session_id, Some(job_id));
        assert_eq!(stored.submitted_at, submitted_at);
        assert_eq!(stored.started_at, submitted_at);
    }

    #[tokio::test]
    async fn http_job_terminal_update_is_compare_and_set() {
        let db = setup_http_jobs_table().await;
        let job_id = Uuid::new_v4();
        let writer = InstallerPersistenceService::new(db.clone());

        writer
            .create_http_job(job_id, job_id, Utc::now())
            .await
            .expect("job should be created");

        let output = sample_output(job_id, Uuid::new_v4());
        writer
            .finish_http_job_succeeded(job_id, output.session_id, output.tenant_id, &output)
            .await
            .expect("job should finish");

        assert!(
            writer
                .finish_http_job_failed(job_id, "second terminal transition")
                .await
                .is_err()
        );

        let stored = writer
            .get_http_job(job_id)
            .await
            .expect("job should be readable")
            .expect("job should exist");
        assert_eq!(stored.status, "succeeded");
        assert_eq!(stored.session_id, Some(output.session_id));
        assert_eq!(stored.tenant_id, output.tenant_id);
        assert!(stored.output.is_some());
    }

    #[tokio::test]
    async fn failed_http_job_does_not_persist_executor_error_text() {
        let db = setup_http_jobs_table().await;
        let job_id = Uuid::new_v4();
        let service = InstallerPersistenceService::new(db);

        service
            .create_http_job(job_id, job_id, Utc::now())
            .await
            .expect("job should be created");
        service
            .finish_http_job_failed(
                job_id,
                "postgres://secret-user:super-secret-password@db/internal error",
            )
            .await
            .expect("failure state should persist");

        let stored = service
            .get_http_job(job_id)
            .await
            .expect("job should be readable")
            .expect("job should exist");
        assert_eq!(
            stored.error_message.as_deref(),
            Some("installer apply failed; inspect durable installer receipts for recovery details")
        );
        assert!(
            !stored
                .error_message
                .as_deref()
                .unwrap_or_default()
                .contains("super-secret-password")
        );
    }
}
