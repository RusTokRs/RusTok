use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "registry_governance_events")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub slug: String,
    pub request_id: Option<String>,
    pub release_id: Option<String>,
    pub event_type: String,
    #[sea_orm(column_name = "actor_principal")]
    pub actor: Json,
    #[sea_orm(column_name = "publisher_principal")]
    pub publisher: Option<Json>,
    pub details: Json,
    pub created_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
