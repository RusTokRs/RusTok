use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "registry_module_owners")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub slug: String,
    #[sea_orm(column_name = "owner_principal")]
    pub owner_principal: Json,
    #[sea_orm(column_name = "bound_by_principal")]
    pub bound_by: Json,
    pub bound_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
