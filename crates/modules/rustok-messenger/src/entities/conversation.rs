use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "messenger_conversations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub conversation_type: i16,
    pub title: Option<String>,
    pub entity_type: Option<i16>,
    pub entity_id: Option<Uuid>,
    pub is_encrypted: bool,
    pub created_by: Uuid,
    pub last_message_id: Option<i64>,
    pub last_message_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::member::Entity")]
    Members,
    #[sea_orm(has_many = "super::message::Entity")]
    Messages,
    #[sea_orm(has_many = "super::channel_translation::Entity")]
    Translations,
    #[sea_orm(has_many = "super::cursor::Entity")]
    Cursors,
}

impl Related<super::member::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Members.def()
    }
}

impl Related<super::message::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Messages.def()
    }
}

impl Related<super::channel_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::cursor::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Cursors.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
