use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

/// SeaORM entity for the `user_field_definitions` table.
///
/// Created by the Flex migration helper — all field definition tables are
/// structurally identical, differing only in table name.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "user_field_definitions")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub field_key: String,
    pub field_type: String,
    pub label: Json,
    pub description: Option<Json>,
    pub is_required: bool,
    pub default_value: Option<Json>,
    pub validation: Option<Json>,
    pub position: i32,
    pub is_active: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
