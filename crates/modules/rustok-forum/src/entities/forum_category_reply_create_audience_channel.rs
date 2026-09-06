use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "forum_category_reply_create_audience_channels")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub category_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub channel_slug: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::forum_category_reply_create_audience_policy::Entity",
        from = "Column::CategoryId",
        to = "super::forum_category_reply_create_audience_policy::Column::CategoryId",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Policy,
}

impl Related<super::forum_category_reply_create_audience_policy::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Policy.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
