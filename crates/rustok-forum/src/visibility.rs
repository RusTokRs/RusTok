use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Effective audience floor for a Forum category.
///
/// Category persistence stores only the narrowing `authenticated` override.
/// `public` is the inherited root default and therefore cannot be written below
/// an authenticated ancestor to broaden its audience.
#[derive(
    Debug,
    Clone,
    Copy,
    Default,
    PartialEq,
    Eq,
    Hash,
    EnumIter,
    DeriveActiveEnum,
    Serialize,
    Deserialize,
    ToSchema,
)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ForumCategoryVisibility {
    #[default]
    #[sea_orm(string_value = "public")]
    Public,
    #[sea_orm(string_value = "authenticated")]
    Authenticated,
}

impl ForumCategoryVisibility {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Public => "public",
            Self::Authenticated => "authenticated",
        }
    }

    pub const fn allows(self, is_authenticated: bool) -> bool {
        match self {
            Self::Public => true,
            Self::Authenticated => is_authenticated,
        }
    }
}
