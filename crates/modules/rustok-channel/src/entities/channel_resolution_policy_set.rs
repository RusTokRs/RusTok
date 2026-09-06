use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "channel_resolution_policy_sets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub schema_version: i32,
    pub is_active: bool,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::channel_resolution_policy_rule::Entity")]
    ChannelResolutionPolicyRules,
}

impl Related<super::channel_resolution_policy_rule::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ChannelResolutionPolicyRules.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
