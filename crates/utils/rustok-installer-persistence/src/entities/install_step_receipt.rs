//! Installer step receipt model.

use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "install_step_receipts")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub session_id: Uuid,
    pub step: String,
    pub outcome: String,
    pub input_checksum: String,
    pub diagnostics: Json,
    pub installer_version: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::install_session::Entity",
        from = "Column::SessionId",
        to = "super::install_session::Column::Id"
    )]
    InstallSession,
}

impl Related<super::install_session::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::InstallSession.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
