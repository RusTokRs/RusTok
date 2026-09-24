use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "media_asset_reference_holds")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub reference_id: Uuid,
    pub tenant_id: Uuid,
    pub media_id: Uuid,
    pub owner_module: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
