use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "brands")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub logo_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub website_url: Option<String>,
    pub is_active: bool,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::brand_translation::Entity")]
    Translations,
    #[sea_orm(has_many = "super::brand_product::Entity")]
    BrandProducts,
}

impl Related<super::brand_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::brand_product::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::BrandProducts.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
