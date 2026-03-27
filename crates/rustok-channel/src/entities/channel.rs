use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "channels")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub is_active: bool,
    pub is_default: bool,
    pub status: String,
    pub settings: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::channel_target::Entity")]
    ChannelTargets,
    #[sea_orm(has_many = "super::channel_module_binding::Entity")]
    ChannelModuleBindings,
    #[sea_orm(has_many = "super::channel_oauth_app::Entity")]
    ChannelOauthApps,
}

impl Related<super::channel_target::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChannelTargets.def()
    }
}

impl Related<super::channel_module_binding::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChannelModuleBindings.def()
    }
}

impl Related<super::channel_oauth_app::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChannelOauthApps.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
