use chrono::Utc;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tracing::instrument;
use uuid::Uuid;

use crate::dto::{ProfileStatus, ProfileSummary, UpsertProfileInput};
use crate::entities::{self, ProfileRecord};
use crate::error::{ProfileError, ProfileResult};

const DEFAULT_PROFILE_LOCALE: &str = "en";
const MIN_HANDLE_LENGTH: usize = 3;
const MAX_HANDLE_LENGTH: usize = 32;
const MAX_LOCALE_LENGTH: usize = 16;
const RESERVED_HANDLES: &[&str] = &["admin", "api", "me", "root", "support", "system"];

impl ProfileService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn normalize_handle(handle: &str) -> ProfileResult<String> {
        let normalized = handle.trim().to_ascii_lowercase();
        if normalized.is_empty() {
            return Err(ProfileError::EmptyHandle);
        }

        if normalized.len() < MIN_HANDLE_LENGTH {
            return Err(ProfileError::HandleTooShort);
        }

        if normalized.len() > MAX_HANDLE_LENGTH {
            return Err(ProfileError::HandleTooLong);
        }

        if !normalized
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
        {
            return Err(ProfileError::InvalidHandle);
        }

        if RESERVED_HANDLES.contains(&normalized.as_str()) {
            return Err(ProfileError::ReservedHandle(normalized));
        }

        Ok(normalized)
    }

    pub fn normalize_locale(locale: Option<&str>) -> ProfileResult<Option<String>> {
        let Some(locale) = locale else {
            return Ok(None);
        };

        let normalized = locale.trim().replace('_', "-").to_ascii_lowercase();
        if normalized.is_empty() || normalized.len() > MAX_LOCALE_LENGTH {
            return Err(ProfileError::InvalidLocale(locale.to_string()));
        }

        if !normalized
            .chars()
            .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '-')
        {
            return Err(ProfileError::InvalidLocale(locale.to_string()));
        }

        Ok(Some(normalized))
    }

    pub fn locale_candidates(
        requested_locale: Option<&str>,
        preferred_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Vec<String>> {
        let mut locales = Vec::new();
        for candidate in [requested_locale, preferred_locale, tenant_default_locale] {
            if let Some(locale) = Self::normalize_locale(candidate)? {
                if !locales.contains(&locale) {
                    locales.push(locale);
                }
            }
        }
        Ok(locales)
    }

    #[instrument(skip(self, input), fields(tenant_id = %tenant_id, user_id = %user_id))]
    pub async fn upsert_profile(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        input: UpsertProfileInput,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let handle = Self::normalize_handle(&input.handle)?;
        let preferred_locale = Self::normalize_locale(input.preferred_locale.as_deref())?;
        let normalized_tenant_default_locale = Self::normalize_locale(tenant_default_locale)?;
        let translation_locale = preferred_locale
            .clone()
            .or(normalized_tenant_default_locale.clone())
            .unwrap_or_else(|| DEFAULT_PROFILE_LOCALE.to_string());

        self.ensure_handle_available(tenant_id, &handle, Some(user_id))
            .await?;

        let existing = entities::profile::Entity::find_by_id(user_id)
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?;

        let now = Utc::now();
        match existing {
            Some(profile) => {
                let mut active: entities::profile::ActiveModel = profile.into();
                active.handle = Set(handle.clone());
                active.display_name = Set(input.display_name.clone());
                active.avatar_media_id = Set(input.avatar_media_id);
                active.banner_media_id = Set(input.banner_media_id);
                active.preferred_locale = Set(preferred_locale.clone());
                active.visibility = Set(input.visibility);
                active.status = Set(ProfileStatus::Active);
                active.updated_at = Set(now.into());
                active.update(&self.db).await?;
            }
            None => {
                entities::profile::ActiveModel {
                    user_id: Set(user_id),
                    tenant_id: Set(tenant_id),
                    handle: Set(handle.clone()),
                    display_name: Set(input.display_name.clone()),
                    avatar_media_id: Set(input.avatar_media_id),
                    banner_media_id: Set(input.banner_media_id),
                    preferred_locale: Set(preferred_locale.clone()),
                    visibility: Set(input.visibility),
                    status: Set(ProfileStatus::Active),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&self.db)
                .await?;
            }
        }

        self.upsert_translation(
            user_id,
            &translation_locale,
            &input.display_name,
            input.bio.as_deref(),
        )
        .await?;

        self.get_profile(tenant_id, user_id, None, tenant_default_locale)
            .await
    }

    pub async fn get_profile(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let profile = entities::profile::Entity::find_by_id(user_id)
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ProfileError::ProfileNotFound(user_id))?;

        let translation = self
            .resolve_translation(
                user_id,
                requested_locale,
                profile.preferred_locale.as_deref(),
                tenant_default_locale,
            )
            .await?;

        Ok(map_profile(profile, translation))
    }

    pub async fn get_profile_by_handle(
        &self,
        tenant_id: Uuid,
        handle: &str,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let normalized_handle = Self::normalize_handle(handle)?;
        let profile = entities::profile::Entity::find()
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .filter(entities::profile::Column::Handle.eq(normalized_handle.clone()))
            .one(&self.db)
            .await?
            .ok_or(ProfileError::ProfileByHandleNotFound(normalized_handle))?;

        let translation = self
            .resolve_translation(
                profile.user_id,
                requested_locale,
                profile.preferred_locale.as_deref(),
                tenant_default_locale,
            )
            .await?;

        Ok(map_profile(profile, translation))
    }

    pub async fn get_profile_summary(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileSummary> {
        let profile = self
            .get_profile(tenant_id, user_id, requested_locale, tenant_default_locale)
            .await?;
        Ok(ProfileSummary {
            user_id: profile.user_id,
            handle: profile.handle,
            display_name: profile.display_name,
            avatar_media_id: profile.avatar_media_id,
            preferred_locale: profile.preferred_locale,
            visibility: profile.visibility,
        })
    }

    pub async fn get_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Vec<ProfileSummary>> {
        let mut profiles = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            profiles.push(
                self.get_profile_summary(
                    tenant_id,
                    *user_id,
                    requested_locale,
                    tenant_default_locale,
                )
                .await?,
            );
        }
        Ok(profiles)
    }

    async fn ensure_handle_available(
        &self,
        tenant_id: Uuid,
        handle: &str,
        except_user_id: Option<Uuid>,
    ) -> ProfileResult<()> {
        let existing = entities::profile::Entity::find()
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .filter(entities::profile::Column::Handle.eq(handle))
            .one(&self.db)
            .await?;

        if let Some(existing) = existing {
            if Some(existing.user_id) != except_user_id {
                return Err(ProfileError::DuplicateHandle(handle.to_string()));
            }
        }

        Ok(())
    }

    async fn upsert_translation(
        &self,
        user_id: Uuid,
        locale: &str,
        display_name: &str,
        bio: Option<&str>,
    ) -> ProfileResult<()> {
        let normalized_locale = Self::normalize_locale(Some(locale))?
            .unwrap_or_else(|| DEFAULT_PROFILE_LOCALE.to_string());
        let now = Utc::now();
        let existing = entities::profile_translation::Entity::find()
            .filter(entities::profile_translation::Column::ProfileUserId.eq(user_id))
            .filter(entities::profile_translation::Column::Locale.eq(normalized_locale.clone()))
            .one(&self.db)
            .await?;

        match existing {
            Some(translation) => {
                let mut active: entities::profile_translation::ActiveModel = translation.into();
                active.display_name = Set(display_name.to_string());
                active.bio = Set(bio.map(str::to_string));
                active.updated_at = Set(now.into());
                active.update(&self.db).await?;
            }
            None => {
                entities::profile_translation::ActiveModel {
                    id: Set(Uuid::new_v4()),
                    profile_user_id: Set(user_id),
                    locale: Set(normalized_locale),
                    display_name: Set(display_name.to_string()),
                    bio: Set(bio.map(str::to_string)),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&self.db)
                .await?;
            }
        }

        Ok(())
    }

    async fn resolve_translation(
        &self,
        user_id: Uuid,
        requested_locale: Option<&str>,
        preferred_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Option<entities::profile_translation::Model>> {
        let candidates =
            Self::locale_candidates(requested_locale, preferred_locale, tenant_default_locale)?;

        for locale in candidates {
            let translation = entities::profile_translation::Entity::find()
                .filter(entities::profile_translation::Column::ProfileUserId.eq(user_id))
                .filter(entities::profile_translation::Column::Locale.eq(locale))
                .one(&self.db)
                .await?;

            if translation.is_some() {
                return Ok(translation);
            }
        }

        Ok(None)
    }
}

#[derive(Debug, Clone)]
pub struct ProfileService {
    db: DatabaseConnection,
}

fn map_profile(
    profile: entities::profile::Model,
    translation: Option<entities::profile_translation::Model>,
) -> ProfileRecord {
    let (display_name, bio) = match translation {
        Some(translation) => (translation.display_name, translation.bio),
        None => (profile.display_name.clone(), None),
    };

    ProfileRecord {
        tenant_id: profile.tenant_id,
        user_id: profile.user_id,
        handle: profile.handle,
        display_name,
        bio,
        avatar_media_id: profile.avatar_media_id,
        banner_media_id: profile.banner_media_id,
        preferred_locale: profile.preferred_locale,
        visibility: profile.visibility,
        status: profile.status,
    }
}

#[cfg(test)]
mod tests {
    use super::ProfileService;
    use crate::error::ProfileError;

    #[test]
    fn normalize_handle_lowercases_and_trims() {
        let normalized = ProfileService::normalize_handle("  Team-Lead_01 ").unwrap();
        assert_eq!(normalized, "team-lead_01");
    }

    #[test]
    fn normalize_handle_rejects_empty_values() {
        let err = ProfileService::normalize_handle("   ").unwrap_err();
        assert!(matches!(err, ProfileError::EmptyHandle));
    }

    #[test]
    fn normalize_handle_rejects_invalid_characters() {
        let err = ProfileService::normalize_handle("bad handle").unwrap_err();
        assert!(matches!(err, ProfileError::InvalidHandle));
    }

    #[test]
    fn normalize_handle_rejects_reserved_values() {
        let err = ProfileService::normalize_handle("Admin").unwrap_err();
        assert!(matches!(err, ProfileError::ReservedHandle(_)));
    }

    #[test]
    fn locale_candidates_keep_priority_and_drop_duplicates() {
        let locales =
            ProfileService::locale_candidates(Some("EN"), Some("ru"), Some("en")).unwrap();
        assert_eq!(locales, vec!["en".to_string(), "ru".to_string()]);
    }
}
