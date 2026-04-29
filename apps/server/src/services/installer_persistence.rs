use chrono::Utc;
use rustok_installer::{redact_install_plan, InstallPlan, InstallReceipt, InstallState};
use sea_orm::{
    ActiveModelTrait, ActiveValue::Set, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder,
};
use uuid::Uuid;

use crate::models::{install_session, install_step_receipt};

#[derive(Clone)]
pub struct InstallerPersistenceService {
    db: DatabaseConnection,
}

impl InstallerPersistenceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
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
        let now = Utc::now();
        if let Some(existing) = install_session::Entity::find()
            .filter(install_session::Column::Id.ne(session.id))
            .filter(install_session::Column::LockExpiresAt.gt(now))
            .filter(install_session::Column::Status.is_not_in(final_state_values()))
            .order_by_desc(install_session::Column::LockExpiresAt)
            .one(&self.db)
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
        active.update(&self.db).await
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
        InstallState::Verified => "verified",
        InstallState::Completed => "completed",
        InstallState::Failed => "failed",
        InstallState::RolledBackFreshInstall => "rolled_back_fresh_install",
        InstallState::RestoreRequired => "restore_required",
    }
}

fn final_state_values() -> Vec<&'static str> {
    vec![
        install_state_value(InstallState::Completed),
        install_state_value(InstallState::Failed),
        install_state_value(InstallState::RolledBackFreshInstall),
        install_state_value(InstallState::RestoreRequired),
    ]
}

fn serde_name<T: serde::Serialize>(value: T) -> String {
    let json =
        serde_json::to_value(value).expect("installer enum serialization must be infallible");
    json.as_str()
        .expect("installer enum serialization must produce a string")
        .to_string()
}
