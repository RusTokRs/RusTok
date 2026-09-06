use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "media_assets")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub uploaded_by: Option<Uuid>,
    pub upload_session_id: Option<Uuid>,
    pub active_blob_id: Option<Uuid>,
    pub original_name: String,
    pub lifecycle_state: String,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub delete_requested_at: Option<DateTimeWithTimeZone>,
    pub deleted_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::blob::Entity",
        from = "Column::ActiveBlobId",
        to = "super::blob::Column::Id"
    )]
    ActiveBlob,
    #[sea_orm(has_many = "super::blob::Entity")]
    BlobHistory,
    #[sea_orm(has_many = "super::rendition::Entity")]
    Renditions,
    #[sea_orm(has_many = "super::media_translation::Entity")]
    Translations,
}

impl Related<super::blob::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::ActiveBlob.def()
    }
}

impl Related<super::rendition::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Renditions.def()
    }
}

impl Related<super::media_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
