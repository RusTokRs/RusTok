use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "product_image_translations")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub image_id: Uuid,
    pub locale: String,
    pub alt_text: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::product_image::Entity",
        from = "Column::ImageId",
        to = "super::product_image::Column::Id"
    )]
    Image,
}

impl Related<super::product_image::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Image.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
