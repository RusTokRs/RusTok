use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "channel_resolution_policy_rules")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub policy_set_id: Uuid,
    pub priority: i32,
    pub is_active: bool,
    pub action_channel_id: Uuid,
    pub definition: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::channel_resolution_policy_set::Entity",
        from = "Column::PolicySetId",
        to = "super::channel_resolution_policy_set::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    PolicySet,
    #[sea_orm(
        belongs_to = "super::channel::Entity",
        from = "Column::ActionChannelId",
        to = "super::channel::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    ActionChannel,
}

impl Related<super::channel_resolution_policy_set::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::PolicySet.def()
    }
}

impl Related<super::channel::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ActionChannel.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
