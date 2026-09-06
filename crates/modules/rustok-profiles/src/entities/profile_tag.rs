use sea_orm::entity::prelude::*;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Eq)]
#[sea_orm(table_name = "profile_tags")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub profile_user_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub term_id: Uuid,
    pub tenant_id: Uuid,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
