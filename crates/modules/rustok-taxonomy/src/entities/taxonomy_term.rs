use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::{TaxonomyScopeType, TaxonomyTermKind};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "taxonomy_terms")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: TaxonomyTermKind,
    pub scope_type: TaxonomyScopeType,
    pub scope_value: String,
    pub canonical_key: String,
    pub revision: i64,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::taxonomy_term_translation::Entity")]
    Translations,
    #[sea_orm(has_many = "super::taxonomy_term_alias::Entity")]
    Aliases,
}

impl Related<super::taxonomy_term_translation::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Translations.def()
    }
}

impl Related<super::taxonomy_term_alias::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Aliases.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
