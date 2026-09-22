use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "taxonomy_term_aliases")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub term_id: Uuid,
    pub tenant_id: Uuid,
    pub locale: String,
    pub name: String,
    pub slug: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::taxonomy_term::Entity",
        from = "Column::TermId",
        to = "super::taxonomy_term::Column::Id",
        on_update = "Cascade",
        on_delete = "Cascade"
    )]
    Term,
}

impl Related<super::taxonomy_term::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Term.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
