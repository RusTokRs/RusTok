use sea_orm::entity::prelude::*;

/// Sealed local legacy-provisioning barrier, not a statement about current Media
/// state. All subsequent owner image inserts also provision coverage atomically.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "product_media_retention_readiness")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
