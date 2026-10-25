use async_graphql::{Enum, SimpleObject};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileSummaryVisibility {
    Public,
    Authenticated,
    FollowersOnly,
    Private,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProfileSummaryAudience {
    Anonymous,
    Authenticated { actor_id: Uuid },
    TrustedService { actor_id: Option<Uuid> },
}

impl ProfileSummaryAudience {
    pub const fn actor_id(self) -> Option<Uuid> {
        match self {
            Self::Anonymous => None,
            Self::Authenticated { actor_id } => Some(actor_id),
            Self::TrustedService { actor_id } => actor_id,
        }
    }

    pub const fn is_authenticated(self) -> bool {
        !matches!(self, Self::Anonymous)
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
    pub visibility: ProfileSummaryVisibility,
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlProfileVisibility {
    Public,
    Authenticated,
    FollowersOnly,
    Private,
}

impl From<ProfileSummaryVisibility> for GqlProfileVisibility {
    fn from(value: ProfileSummaryVisibility) -> Self {
        match value {
            ProfileSummaryVisibility::Public => Self::Public,
            ProfileSummaryVisibility::Authenticated => Self::Authenticated,
            ProfileSummaryVisibility::FollowersOnly => Self::FollowersOnly,
            ProfileSummaryVisibility::Private => Self::Private,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GqlProfileSummary {
    pub user_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub tags: Vec<String>,
    pub avatar_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: GqlProfileVisibility,
}

impl From<ProfileSummary> for GqlProfileSummary {
    fn from(value: ProfileSummary) -> Self {
        Self {
            user_id: value.user_id,
            handle: value.handle,
            display_name: value.display_name,
            tags: value.tags,
            avatar_media_id: value.avatar_media_id,
            preferred_locale: value.preferred_locale,
            visibility: value.visibility.into(),
        }
    }
}
