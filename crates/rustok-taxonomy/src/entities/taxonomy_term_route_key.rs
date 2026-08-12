use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::dto::{TaxonomyScopeType, TaxonomyTermKind};

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "taxonomy_term_route_keys")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub kind: TaxonomyTermKind,
    #[sea_orm(primary_key, auto_increment = false)]
    pub scope_type: TaxonomyScopeType,
    #[sea_orm(primary_key, auto_increment = false)]
    pub scope_value: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub locale: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub route_key: String,
    pub term_id: Uuid,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
