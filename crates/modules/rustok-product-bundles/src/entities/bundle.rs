use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_bundles")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub bundle_product_id: Option<Uuid>,
    pub slug: String,
    pub bundle_type: String,
    pub status: String,
    pub discount_type: String,
    pub discount_value: Decimal,
    pub metadata: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::bundle_translation::Entity")]
    Translations,
    #[sea_orm(has_many = "super::bundle_item::Entity")]
    Items,
}

impl Related<super::bundle_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::bundle_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Items.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
