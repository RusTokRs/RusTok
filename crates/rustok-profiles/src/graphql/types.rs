use async_graphql::{Enum, InputObject, SimpleObject};
use uuid::Uuid;

use crate::{ProfileRecord, ProfileStatus, ProfileSummary, ProfileVisibility, UpsertProfileInput};

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlProfileVisibility {
    Public,
    Authenticated,
    FollowersOnly,
    Private,
}

impl From<ProfileVisibility> for GqlProfileVisibility {
    fn from(value: ProfileVisibility) -> Self {
        match value {
            ProfileVisibility::Public => Self::Public,
            ProfileVisibility::Authenticated => Self::Authenticated,
            ProfileVisibility::FollowersOnly => Self::FollowersOnly,
            ProfileVisibility::Private => Self::Private,
        }
    }
}

impl From<GqlProfileVisibility> for ProfileVisibility {
    fn from(value: GqlProfileVisibility) -> Self {
        match value {
            GqlProfileVisibility::Public => Self::Public,
            GqlProfileVisibility::Authenticated => Self::Authenticated,
            GqlProfileVisibility::FollowersOnly => Self::FollowersOnly,
            GqlProfileVisibility::Private => Self::Private,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
pub enum GqlProfileStatus {
    Active,
    Hidden,
    Blocked,
}

impl From<ProfileStatus> for GqlProfileStatus {
    fn from(value: ProfileStatus) -> Self {
        match value {
            ProfileStatus::Active => Self::Active,
            ProfileStatus::Hidden => Self::Hidden,
            ProfileStatus::Blocked => Self::Blocked,
        }
    }
}

#[derive(SimpleObject, Debug, Clone)]
pub struct GqlProfile {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub tags: Vec<String>,
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: GqlProfileVisibility,
    pub status: GqlProfileStatus,
}

impl From<ProfileRecord> for GqlProfile {
    fn from(value: ProfileRecord) -> Self {
        Self {
            tenant_id: value.tenant_id,
            user_id: value.user_id,
            handle: value.handle,
            display_name: value.display_name,
            bio: value.bio,
            tags: value.tags,
            avatar_media_id: value.avatar_media_id,
            banner_media_id: value.banner_media_id,
            preferred_locale: value.preferred_locale,
            visibility: value.visibility.into(),
            status: value.status.into(),
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

#[derive(InputObject, Debug, Clone)]
pub struct GqlUpsertProfileInput {
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub tags: Option<Vec<String>>,
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: GqlProfileVisibility,
}

impl From<GqlUpsertProfileInput> for UpsertProfileInput {
    fn from(value: GqlUpsertProfileInput) -> Self {
        Self {
            handle: value.handle,
            display_name: value.display_name,
            bio: value.bio,
            tags: value.tags.unwrap_or_default(),
            avatar_media_id: value.avatar_media_id,
            banner_media_id: value.banner_media_id,
            preferred_locale: value.preferred_locale,
            visibility: value.visibility.into(),
        }
    }
}

#[derive(InputObject, Debug, Clone)]
pub struct GqlUpdateMyProfileContentInput {
    pub display_name: String,
    pub bio: Option<String>,
}

#[derive(InputObject, Debug, Clone)]
pub struct GqlUpdateMyProfileMediaInput {
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
}
