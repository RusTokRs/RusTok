use sea_orm::entity::prelude::*;

/// Product-local provisioning marker, NOT a mirror of Media's current hold state.
/// Pre-cutover rows have no marker and block new image admission until backfilled.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "product_image_media_coverage")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub image_id: Uuid,
    pub tenant_id: Uuid,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
