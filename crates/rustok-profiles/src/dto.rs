use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ProfileVisibility {
    #[sea_orm(string_value = "public")]
    Public,
    #[sea_orm(string_value = "authenticated")]
    Authenticated,
    #[sea_orm(string_value = "followers_only")]
    FollowersOnly,
    #[sea_orm(string_value = "private")]
    Private,
}

impl std::fmt::Display for ProfileVisibility {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "public"),
            Self::Authenticated => write!(f, "authenticated"),
            Self::FollowersOnly => write!(f, "followers_only"),
            Self::Private => write!(f, "private"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumIter, DeriveActiveEnum, Serialize, Deserialize)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::N(32))")]
#[serde(rename_all = "snake_case")]
pub enum ProfileStatus {
    #[sea_orm(string_value = "active")]
    Active,
    #[sea_orm(string_value = "hidden")]
    Hidden,
    #[sea_orm(string_value = "blocked")]
    Blocked,
}

impl std::fmt::Display for ProfileStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Hidden => write!(f, "hidden"),
            Self::Blocked => write!(f, "blocked"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub user_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub tags: Vec<String>,
    pub avatar_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: ProfileVisibility,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertProfileInput {
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub tags: Vec<String>,
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: ProfileVisibility,
}
