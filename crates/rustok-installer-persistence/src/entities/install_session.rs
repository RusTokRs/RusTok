//! Installer session model.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "install_sessions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Option<Uuid>,
    pub status: String,
    pub profile: String,
    pub environment: String,
    pub database_engine: String,
    pub seed_profile: String,
    pub plan_snapshot: Json,
    pub lock_owner: Option<String>,
    pub lock_expires_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::install_step_receipt::Entity")]
    InstallStepReceipts,
}

impl Related<super::install_step_receipt::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InstallStepReceipts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
