use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "flex_schema_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub schema_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub locale: String,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::flex_schemas::Entity",
        from = "Column::SchemaId",
        to = "super::flex_schemas::Column::Id"
    )]
    FlexSchema,
}

impl Related<super::flex_schemas::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::FlexSchema.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
