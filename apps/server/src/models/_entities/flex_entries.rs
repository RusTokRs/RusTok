use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "flex_entries")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub schema_id: Uuid,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub data: Json,
    pub status: String,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::tenants::Entity",
        from = "Column::TenantId",
        to = "super::tenants::Column::Id"
    )]
    Tenant,
    #[sea_orm(
        belongs_to = "super::flex_schemas::Entity",
        from = "Column::SchemaId",
        to = "super::flex_schemas::Column::Id"
    )]
    FlexSchema,
    #[sea_orm(has_many = "super::flex_entry_localized_values::Entity")]
    FlexEntryLocalizedValues,
}

impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl Related<super::flex_schemas::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FlexSchema.def()
    }
}

impl Related<super::flex_entry_localized_values::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FlexEntryLocalizedValues.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
