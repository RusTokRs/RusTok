use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Site-level symbol definition body for one `(tenant, locale)` catalog.
///
/// The composite key is the identity contract: a symbol id is unique per
/// tenant and locale, matching how page bodies are localized. The `content`
/// column stores the full [`fly::SymbolDescriptor`] JSON value authored in the
/// builder document's `flySymbols` block.
#[derive(Clone, Debug, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "site_symbols")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub tenant_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub locale: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub symbol_id: String,
    pub name: Option<String>,
    pub content: Json,
    pub created_at: DateTimeWithTimeZone,
    pub updated_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {}

impl ActiveModelBehavior for ActiveModel {}
