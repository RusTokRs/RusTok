use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "flex_schemas")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub fields_config: Json,
    pub settings: Json,
    pub is_active: bool,
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
    #[sea_orm(has_many = "super::flex_entries::Entity")]
    FlexEntries,
    #[sea_orm(has_many = "super::flex_schema_translations::Entity")]
    FlexSchemaTranslations,
}

impl Related<super::tenants::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Tenant.def()
    }
}

impl Related<super::flex_entries::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FlexEntries.def()
    }
}

impl Related<super::flex_schema_translations::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FlexSchemaTranslations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
