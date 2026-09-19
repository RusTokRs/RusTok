use sea_orm::entity::prelude::*;
use crate::domain::ProductStatus;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "products")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub status: ProductStatus,
    pub seller_id: Option<String>,
    pub vendor: Option<String>,
    pub product_type: Option<String>,
    pub shipping_profile_slug: Option<String>,
    pub primary_category_id: Option<Uuid>,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
    pub published_at: Option<DateTimeWithTimeZone>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::product_translation::Entity")]
    Translations,
    #[sea_orm(has_many = "super::product_variant::Entity")]
    Variants,
    #[sea_orm(has_many = "super::product_variant_axis::Entity")]
    VariantAxes,
    #[sea_orm(has_many = "super::product_image::Entity")]
    Images,
}

impl Related<super::product_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::product_variant::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Variants.def()
    }
}

impl Related<super::product_variant_axis::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::VariantAxes.def()
    }
}

impl Related<super::product_image::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Images.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
