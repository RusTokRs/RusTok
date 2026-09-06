use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "translation_glossary_terms")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub glossary_id: Uuid,
    pub concept_key: String,
    pub source_term: String,
    pub target_term: String,
    pub policy: String,
    pub match_kind: String,
    pub case_sensitive: bool,
    pub notes: String,
    pub valid_from_revision: i64,
    pub valid_to_revision: Option<i64>,
    pub created_by_actor_kind: String,
    pub created_by_actor_id: String,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
