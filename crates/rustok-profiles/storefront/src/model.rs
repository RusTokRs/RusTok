use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilesStorefrontImage {
    pub url: String,
    pub alt: Option<String>,
    pub width: Option<i32>,
    pub height: Option<i32>,
    pub mime_type: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilesStorefrontProfile {
    pub user_id: String,
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub tags: Vec<String>,
    pub avatar_media_id: Option<String>,
    pub banner_media_id: Option<String>,
    pub avatar_image: Option<ProfilesStorefrontImage>,
    pub banner_image: Option<ProfilesStorefrontImage>,
    pub preferred_locale: Option<String>,
    pub visibility: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilesStorefrontFollowState {
    pub user_id: String,
    pub following: bool,
    pub revision: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfilesStorefrontPage {
    pub profile: Option<ProfilesStorefrontProfile>,
    pub viewer_authenticated: bool,
    pub is_self: bool,
    pub follow_state: Option<ProfilesStorefrontFollowState>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetProfilesStorefrontFollowCommand {
    pub user_id: String,
    pub following: bool,
    pub expected_revision: Option<String>,
    pub idempotency_key: String,
}
