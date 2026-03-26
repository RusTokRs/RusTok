use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

use crate::dto::{ProfileStatus, ProfileVisibility};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "profiles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub user_id: Uuid,
    pub tenant_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: ProfileVisibility,
    pub status: ProfileStatus,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::profile_translation::Entity")]
    Translations,
}

impl Related<super::profile_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
