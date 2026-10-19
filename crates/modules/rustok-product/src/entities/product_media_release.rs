use sea_orm::entity::prelude::*;

/// A committed removal, not a speculative orphan-repair instruction. No FK to a
/// deleted Product/Image or to Media: this work must survive their removal.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "product_media_releases")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub image_id: Uuid,
    pub tenant_id: Uuid,
    pub media_id: Uuid,
    pub context: Json,
    pub attempts: i32,
    pub next_attempt_at: DateTimeUtc,
    pub last_error_code: Option<String>,
}
#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}
impl ActiveModelBehavior for ActiveModel {}
