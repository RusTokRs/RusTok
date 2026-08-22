use chrono::{DateTime, Utc};
use rustok_api::TenantLocale;
use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum TaxonomyTermKind {
    #[sea_orm(string_value = "tag")]
    Tag,
    #[sea_orm(string_value = "category")]
    Category,
}

impl std::fmt::Display for TaxonomyTermKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tag => write!(f, "tag"),
            Self::Category => write!(f, "category"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum TaxonomyScopeType {
    #[sea_orm(string_value = "global")]
    Global,
    #[sea_orm(string_value = "module")]
    Module,
}

impl std::fmt::Display for TaxonomyScopeType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Global => write!(f, "global"),
            Self::Module => write!(f, "module"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateTaxonomyTermInput {
    pub kind: TaxonomyTermKind,
    pub scope_type: TaxonomyScopeType,
    pub scope_value: Option<String>,
    pub locale: String,
    pub name: String,
    pub slug: Option<String>,
    pub canonical_key: Option<String>,
    pub description: Option<String>,
    #[serde(default)]
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateTaxonomyTermInput {
    pub locale: String,
    pub name: Option<String>,
    pub slug: Option<String>,
    pub description: Option<String>,
    pub aliases: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolveTaxonomyTermInput {
    pub kind: TaxonomyTermKind,
    pub module_slug: String,
    pub locale: String,
    pub slug_or_alias: String,
    pub fallback_locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetTaxonomyCategoryPlacementInput {
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyCategoryPlacement {
    pub term_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub position: i32,
}

/// Strongly typed reference to an asset owned by the Media capability.
///
/// Taxonomy persists only the Media identity. Runtime composition must validate
/// that the referenced asset belongs to the same tenant and is an active,
/// ready public image before admitting a Category presentation write.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TaxonomyCategoryMediaId(Uuid);

impl TaxonomyCategoryMediaId {
    pub const fn new(media_id: Uuid) -> Self {
        Self(media_id)
    }

    pub const fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl From<Uuid> for TaxonomyCategoryMediaId {
    fn from(value: Uuid) -> Self {
        Self::new(value)
    }
}

impl From<TaxonomyCategoryMediaId> for Uuid {
    fn from(value: TaxonomyCategoryMediaId) -> Self {
        value.into_uuid()
    }
}

/// Full canonical Category presentation replacement.
///
/// `expected_revision` is a presentation-only optimistic concurrency token.
/// It is intentionally separate from the Taxonomy term/Translation resource
/// revision so changing icon/color/media does not invalidate a text translation
/// proposal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetTaxonomyCategoryPresentationInput {
    pub icon_key: Option<String>,
    pub color: Option<String>,
    pub image_media_id: Option<TaxonomyCategoryMediaId>,
    pub cover_media_id: Option<TaxonomyCategoryMediaId>,
    pub expected_revision: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyCategoryPresentation {
    pub term_id: Uuid,
    pub icon_key: Option<String>,
    pub color: Option<String>,
    pub image_media_id: Option<TaxonomyCategoryMediaId>,
    pub cover_media_id: Option<TaxonomyCategoryMediaId>,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyExactTaxonomyTranslationInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub expected_resource_revision: i64,
    pub expected_source_revision: i64,
    pub expected_target_revision: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TaxonomyTranslationApplyResult {
    pub resource_revision: i64,
    pub target_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyTermResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub kind: TaxonomyTermKind,
    pub scope_type: TaxonomyScopeType,
    pub scope_value: Option<String>,
    pub canonical_key: String,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub aliases: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaxonomyTermListItem {
    pub id: Uuid,
    pub kind: TaxonomyTermKind,
    pub scope_type: TaxonomyScopeType,
    pub scope_value: Option<String>,
    pub canonical_key: String,
    pub requested_locale: String,
    pub effective_locale: String,
    pub available_locales: Vec<String>,
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ListTaxonomyTermsFilter {
    pub kind: Option<TaxonomyTermKind>,
    pub scope_type: Option<TaxonomyScopeType>,
    pub scope_value: Option<String>,
    pub locale: Option<String>,
    pub page: Option<u64>,
    pub per_page: Option<u64>,
}

impl ListTaxonomyTermsFilter {
    pub fn page(&self) -> u64 {
        self.page.unwrap_or(1).max(1)
    }

    pub fn per_page(&self) -> u64 {
        self.per_page.unwrap_or(20).max(1)
    }
}
