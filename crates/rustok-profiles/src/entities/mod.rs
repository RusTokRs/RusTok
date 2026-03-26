use crate::dto::{ProfileStatus, ProfileVisibility};
use uuid::Uuid;

pub mod profile;
pub mod profile_translation;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileRecord {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub handle: String,
    pub display_name: String,
    pub bio: Option<String>,
    pub avatar_media_id: Option<Uuid>,
    pub banner_media_id: Option<Uuid>,
    pub preferred_locale: Option<String>,
    pub visibility: ProfileVisibility,
    pub status: ProfileStatus,
}
