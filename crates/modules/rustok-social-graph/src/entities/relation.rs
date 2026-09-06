use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::model::SocialRelationKind;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "social_graph_relations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_user_id: Uuid,
    pub target_user_id: Uuid,
    pub relation_kind: SocialRelationKind,
    pub active: bool,
    pub revision: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
