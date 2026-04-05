use chrono::Utc;
use rustok_core::normalize_locale_tag;
use rustok_taxonomy::{TaxonomyService, TaxonomyTermKind};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use std::collections::HashMap;
use tracing::instrument;
use uuid::Uuid;

use crate::dto::{ProfileStatus, ProfileSummary, ProfileVisibility, UpsertProfileInput};
use crate::entities::{self, ProfileRecord};
use crate::error::{ProfileError, ProfileResult};

const DEFAULT_PROFILE_LOCALE: &str = "en";
const PROFILE_SCOPE_VALUE: &str = "profiles";
const MIN_HANDLE_LENGTH: usize = 3;
const MAX_HANDLE_LENGTH: usize = 32;
const MAX_DISPLAY_NAME_LENGTH: usize = 255;
const MAX_LOCALE_LENGTH: usize = 16;
const MAX_HANDLE_SUFFIX_ATTEMPTS: usize = 100;
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

    pub fn normalize_display_name(display_name: &str) -> ProfileResult<String> {
        let normalized = display_name.trim();
        if normalized.is_empty() {
            return Err(ProfileError::EmptyDisplayName);
        }

        if normalized.chars().count() > MAX_DISPLAY_NAME_LENGTH {
            return Err(ProfileError::DisplayNameTooLong);
        }

        Ok(normalized.to_string())
    }

    pub fn normalize_locale(locale: Option<&str>) -> ProfileResult<Option<String>> {
        let Some(locale) = locale else {
            return Ok(None);
        };

        let Some(normalized) = normalize_locale_tag(locale) else {
            return Err(ProfileError::InvalidLocale(locale.to_string()));
        };
        if normalized.len() > MAX_LOCALE_LENGTH {
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
        let display_name = Self::normalize_display_name(&input.display_name)?;
        let preferred_locale = Self::normalize_locale(input.preferred_locale.as_deref())?;
        let normalized_tenant_default_locale = Self::normalize_locale(tenant_default_locale)?;
        let translation_locale = preferred_locale
            .clone()
            .or(normalized_tenant_default_locale.clone())
            .unwrap_or_else(|| DEFAULT_PROFILE_LOCALE.to_string());

        self.ensure_handle_available(tenant_id, &handle, Some(user_id))
            .await?;

        let txn = self.db.begin().await?;
        let existing = entities::profile::Entity::find_by_id(user_id)
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .one(&txn)
            .await?;

        let now = Utc::now();
        match existing {
            Some(profile) => {
                let mut active: entities::profile::ActiveModel = profile.into();
                active.handle = Set(handle.clone());
                active.display_name = Set(display_name.clone());
                active.avatar_media_id = Set(input.avatar_media_id);
                active.banner_media_id = Set(input.banner_media_id);
                active.preferred_locale = Set(preferred_locale.clone());
                active.visibility = Set(input.visibility);
                active.status = Set(ProfileStatus::Active);
                active.updated_at = Set(now.into());
                active.update(&txn).await?;
            }
            None => {
                entities::profile::ActiveModel {
                    user_id: Set(user_id),
                    tenant_id: Set(tenant_id),
                    handle: Set(handle.clone()),
                    display_name: Set(display_name.clone()),
                    avatar_media_id: Set(input.avatar_media_id),
                    banner_media_id: Set(input.banner_media_id),
                    preferred_locale: Set(preferred_locale.clone()),
                    visibility: Set(input.visibility),
                    status: Set(ProfileStatus::Active),
                    created_at: Set(now.into()),
                    updated_at: Set(now.into()),
                }
                .insert(&txn)
                .await?;
            }
        }

        self.upsert_translation_in_conn(
            &txn,
            user_id,
            &translation_locale,
            &display_name,
            input.bio.as_deref(),
        )
        .await?;
        self.sync_profile_tags_in_tx(&txn, tenant_id, user_id, &translation_locale, &input.tags)
            .await?;
        txn.commit().await?;

        self.get_profile(tenant_id, user_id, None, tenant_default_locale)
            .await
    }

    pub async fn update_profile_handle(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        handle: &str,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let handle = Self::normalize_handle(handle)?;
        self.ensure_handle_available(tenant_id, &handle, Some(user_id))
            .await?;

        let profile = self.get_existing_profile_model(tenant_id, user_id).await?;
        let mut active: entities::profile::ActiveModel = profile.into();
        active.handle = Set(handle);
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.get_profile(tenant_id, user_id, None, tenant_default_locale)
            .await
    }

    pub async fn update_profile_content(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        display_name: &str,
        bio: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let display_name = Self::normalize_display_name(display_name)?;
        let profile = self.get_existing_profile_model(tenant_id, user_id).await?;
        let translation_locale =
            resolve_profile_translation_locale(&profile, tenant_default_locale)?;

        let mut active: entities::profile::ActiveModel = profile.clone().into();
        active.display_name = Set(display_name.clone());
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.upsert_translation(user_id, &translation_locale, &display_name, bio)
            .await?;

        self.get_profile(
            tenant_id,
            user_id,
            Some(translation_locale.as_str()),
            tenant_default_locale,
        )
        .await
    }

    pub async fn update_profile_locale(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        preferred_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let current_profile = self
            .get_profile(tenant_id, user_id, None, tenant_default_locale)
            .await?;
        let preferred_locale = Self::normalize_locale(preferred_locale)?;
        let profile = self.get_existing_profile_model(tenant_id, user_id).await?;
        let translation_locale = preferred_locale
            .clone()
            .or(Self::normalize_locale(tenant_default_locale)?)
            .unwrap_or_else(|| DEFAULT_PROFILE_LOCALE.to_string());

        let mut active: entities::profile::ActiveModel = profile.clone().into();
        active.preferred_locale = Set(preferred_locale);
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.upsert_translation(
            user_id,
            &translation_locale,
            &current_profile.display_name,
            current_profile.bio.as_deref(),
        )
        .await?;

        self.get_profile(
            tenant_id,
            user_id,
            Some(translation_locale.as_str()),
            tenant_default_locale,
        )
        .await
    }

    pub async fn update_profile_visibility(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        visibility: crate::dto::ProfileVisibility,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let profile = self.get_existing_profile_model(tenant_id, user_id).await?;
        let mut active: entities::profile::ActiveModel = profile.into();
        active.visibility = Set(visibility);
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.get_profile(tenant_id, user_id, None, tenant_default_locale)
            .await
    }

    pub async fn update_profile_media(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        avatar_media_id: Option<Uuid>,
        banner_media_id: Option<Uuid>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let profile = self.get_existing_profile_model(tenant_id, user_id).await?;
        let mut active: entities::profile::ActiveModel = profile.into();
        active.avatar_media_id = Set(avatar_media_id);
        active.banner_media_id = Set(banner_media_id);
        active.updated_at = Set(Utc::now().into());
        active.update(&self.db).await?;

        self.get_profile(tenant_id, user_id, None, tenant_default_locale)
            .await
    }

    pub async fn plan_backfill_profile(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        email: &str,
        display_name: Option<&str>,
        preferred_locale: Option<&str>,
        visibility: ProfileVisibility,
    ) -> ProfileResult<UpsertProfileInput> {
        let normalized_display_name = resolve_backfill_display_name(display_name, email, user_id)?;
        let handle = self
            .generate_available_handle(tenant_id, &normalized_display_name, email, user_id)
            .await?;

        Ok(UpsertProfileInput {
            handle,
            display_name: normalized_display_name,
            bio: None,
            tags: Vec::new(),
            avatar_media_id: None,
            banner_media_id: None,
            preferred_locale: Self::normalize_locale(preferred_locale)?,
            visibility,
        })
    }

    pub async fn backfill_profile(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        email: &str,
        display_name: Option<&str>,
        preferred_locale: Option<&str>,
        visibility: ProfileVisibility,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileBackfillResult> {
        match self
            .get_profile(tenant_id, user_id, preferred_locale, tenant_default_locale)
            .await
        {
            Ok(profile) => {
                return Ok(ProfileBackfillResult {
                    profile,
                    created: false,
                });
            }
            Err(ProfileError::ProfileNotFound(_)) => {}
            Err(error) => return Err(error),
        }

        let input = self
            .plan_backfill_profile(
                tenant_id,
                user_id,
                email,
                display_name,
                preferred_locale,
                visibility,
            )
            .await?;
        let profile = self
            .upsert_profile(tenant_id, user_id, input, tenant_default_locale)
            .await?;

        Ok(ProfileBackfillResult {
            profile,
            created: true,
        })
    }

    pub async fn get_profile(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        self.find_profile_records_map(
            tenant_id,
            &[user_id],
            requested_locale,
            tenant_default_locale,
        )
        .await?
        .remove(&user_id)
        .ok_or(ProfileError::ProfileNotFound(user_id))
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
        let profile_tags = self
            .load_profile_tag_map(
                tenant_id,
                std::slice::from_ref(&profile),
                requested_locale,
                tenant_default_locale,
            )
            .await?;

        Ok(map_profile(
            profile.clone(),
            translation,
            profile_tags
                .get(&profile.user_id)
                .cloned()
                .unwrap_or_default(),
        ))
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
            tags: profile.tags,
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
        let profiles = self
            .find_profile_summaries_map(
                tenant_id,
                user_ids,
                requested_locale,
                tenant_default_locale,
            )
            .await?;
        let mut summaries = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            summaries.push(
                profiles
                    .get(user_id)
                    .cloned()
                    .ok_or(ProfileError::ProfileNotFound(*user_id))?,
            );
        }
        Ok(summaries)
    }

    pub(crate) async fn find_profile_summaries_map(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<Uuid, ProfileSummary>> {
        Ok(self
            .find_profile_records_map(tenant_id, user_ids, requested_locale, tenant_default_locale)
            .await?
            .into_iter()
            .map(|(user_id, profile)| {
                (
                    user_id,
                    ProfileSummary {
                        user_id: profile.user_id,
                        handle: profile.handle,
                        display_name: profile.display_name,
                        tags: profile.tags,
                        avatar_media_id: profile.avatar_media_id,
                        preferred_locale: profile.preferred_locale,
                        visibility: profile.visibility,
                    },
                )
            })
            .collect())
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

    async fn generate_available_handle(
        &self,
        tenant_id: Uuid,
        display_name: &str,
        email: &str,
        user_id: Uuid,
    ) -> ProfileResult<String> {
        let fallback = format!("user-{}", short_user_suffix(user_id));
        let mut bases = Vec::new();

        for candidate in [
            slugify_handle_seed(display_name),
            slugify_handle_seed(email_local_part(email)),
            Some(fallback),
        ]
        .into_iter()
        .flatten()
        {
            if !bases.contains(&candidate) {
                bases.push(candidate);
            }
        }

        for base in bases {
            for suffix in 0..MAX_HANDLE_SUFFIX_ATTEMPTS {
                let Some(candidate) = build_handle_candidate(&base, suffix) else {
                    continue;
                };
                match self
                    .ensure_handle_available(tenant_id, &candidate, Some(user_id))
                    .await
                {
                    Ok(()) => return Ok(candidate),
                    Err(ProfileError::DuplicateHandle(_)) => continue,
                    Err(error) => return Err(error),
                }
            }
        }

        Err(ProfileError::DuplicateHandle(format!(
            "user-{}",
            short_user_suffix(user_id)
        )))
    }

    async fn upsert_translation(
        &self,
        user_id: Uuid,
        locale: &str,
        display_name: &str,
        bio: Option<&str>,
    ) -> ProfileResult<()> {
        self.upsert_translation_in_conn(&self.db, user_id, locale, display_name, bio)
            .await
    }

    async fn upsert_translation_in_conn<C>(
        &self,
        conn: &C,
        user_id: Uuid,
        locale: &str,
        display_name: &str,
        bio: Option<&str>,
    ) -> ProfileResult<()>
    where
        C: ConnectionTrait,
    {
        let normalized_locale = Self::normalize_locale(Some(locale))?
            .unwrap_or_else(|| DEFAULT_PROFILE_LOCALE.to_string());
        let now = Utc::now();
        let existing = entities::profile_translation::Entity::find()
            .filter(entities::profile_translation::Column::ProfileUserId.eq(user_id))
            .filter(entities::profile_translation::Column::Locale.eq(normalized_locale.clone()))
            .one(conn)
            .await?;

        match existing {
            Some(translation) => {
                let mut active: entities::profile_translation::ActiveModel = translation.into();
                active.display_name = Set(display_name.to_string());
                active.bio = Set(bio.map(str::to_string));
                active.updated_at = Set(now.into());
                active.update(conn).await?;
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
                .insert(conn)
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

    async fn find_profile_records_map(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<Uuid, ProfileRecord>> {
        let user_ids = unique_user_ids(user_ids);
        if user_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let profiles = entities::profile::Entity::find()
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .filter(entities::profile::Column::UserId.is_in(user_ids))
            .all(&self.db)
            .await?;

        if profiles.is_empty() {
            return Ok(HashMap::new());
        }

        let translations = self
            .load_translations_map(&profiles, requested_locale, tenant_default_locale)
            .await?;
        let tags = self
            .load_profile_tag_map(
                tenant_id,
                &profiles,
                requested_locale,
                tenant_default_locale,
            )
            .await?;

        profiles
            .into_iter()
            .map(|profile| {
                let user_id = profile.user_id;
                let translation = select_translation(
                    &translations,
                    &profile,
                    requested_locale,
                    tenant_default_locale,
                )?;
                Ok((
                    user_id,
                    map_profile(
                        profile,
                        translation,
                        tags.get(&user_id).cloned().unwrap_or_default(),
                    ),
                ))
            })
            .collect()
    }

    async fn load_profile_tag_map(
        &self,
        tenant_id: Uuid,
        profiles: &[entities::profile::Model],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<Uuid, Vec<String>>> {
        if profiles.is_empty() {
            return Ok(HashMap::new());
        }

        let profile_user_ids = profiles
            .iter()
            .map(|profile| profile.user_id)
            .collect::<Vec<_>>();
        let relations = entities::profile_tag::Entity::find()
            .filter(entities::profile_tag::Column::ProfileUserId.is_in(profile_user_ids))
            .order_by_asc(entities::profile_tag::Column::ProfileUserId)
            .order_by_asc(entities::profile_tag::Column::CreatedAt)
            .all(&self.db)
            .await?;

        if relations.is_empty() {
            return Ok(HashMap::new());
        }

        let locale = requested_locale
            .map(|value| Self::normalize_locale(Some(value)))
            .transpose()?
            .flatten()
            .or(Self::normalize_locale(tenant_default_locale)?)
            .unwrap_or_else(|| DEFAULT_PROFILE_LOCALE.to_string());
        let fallback_locale = Self::normalize_locale(tenant_default_locale)?;

        let mut term_ids = Vec::new();
        let mut relations_by_profile: HashMap<Uuid, Vec<entities::profile_tag::Model>> =
            HashMap::new();
        for relation in relations {
            if !term_ids.contains(&relation.term_id) {
                term_ids.push(relation.term_id);
            }
            relations_by_profile
                .entry(relation.profile_user_id)
                .or_default()
                .push(relation);
        }

        let names = TaxonomyService::new(self.db.clone())
            .resolve_term_names(tenant_id, &term_ids, &locale, fallback_locale.as_deref())
            .await?;

        let mut tags_by_profile = HashMap::new();
        for profile in profiles {
            let tags = relations_by_profile
                .get(&profile.user_id)
                .into_iter()
                .flatten()
                .filter_map(|relation| names.get(&relation.term_id).cloned())
                .collect::<Vec<_>>();
            if !tags.is_empty() {
                tags_by_profile.insert(profile.user_id, tags);
            }
        }

        Ok(tags_by_profile)
    }

    async fn sync_profile_tags_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        user_id: Uuid,
        locale: &str,
        tag_names: &[String],
    ) -> ProfileResult<()> {
        let normalized_tags = normalize_tag_names(tag_names);
        entities::profile_tag::Entity::delete_many()
            .filter(entities::profile_tag::Column::ProfileUserId.eq(user_id))
            .exec(txn)
            .await?;

        if normalized_tags.is_empty() {
            return Ok(());
        }

        let term_ids = TaxonomyService::new(self.db.clone())
            .ensure_terms_for_module_in_tx(
                txn,
                tenant_id,
                TaxonomyTermKind::Tag,
                PROFILE_SCOPE_VALUE,
                locale,
                &normalized_tags,
            )
            .await?;

        let now = Utc::now();
        for (index, term_id) in term_ids.into_iter().enumerate() {
            entities::profile_tag::ActiveModel {
                profile_user_id: Set(user_id),
                term_id: Set(term_id),
                tenant_id: Set(tenant_id),
                // Preserve caller-provided tag order in read paths that sort by created_at.
                created_at: Set((now + chrono::Duration::microseconds(index as i64)).into()),
            }
            .insert(txn)
            .await?;
        }

        Ok(())
    }

    async fn load_translations_map(
        &self,
        profiles: &[entities::profile::Model],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<(Uuid, String), entities::profile_translation::Model>> {
        let locales = collect_batch_locales(profiles, requested_locale, tenant_default_locale)?;
        if locales.is_empty() {
            return Ok(HashMap::new());
        }

        let profile_user_ids = profiles
            .iter()
            .map(|profile| profile.user_id)
            .collect::<Vec<_>>();
        let translations = entities::profile_translation::Entity::find()
            .filter(entities::profile_translation::Column::ProfileUserId.is_in(profile_user_ids))
            .filter(entities::profile_translation::Column::Locale.is_in(locales))
            .all(&self.db)
            .await?;

        Ok(translations
            .into_iter()
            .map(|translation| {
                (
                    (translation.profile_user_id, translation.locale.clone()),
                    translation,
                )
            })
            .collect())
    }

    async fn get_existing_profile_model(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
    ) -> ProfileResult<entities::profile::Model> {
        entities::profile::Entity::find_by_id(user_id)
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ProfileError::ProfileNotFound(user_id))
    }
}

#[derive(Debug, Clone)]
pub struct ProfileService {
    db: DatabaseConnection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileBackfillResult {
    pub profile: ProfileRecord,
    pub created: bool,
}

fn unique_user_ids(user_ids: &[Uuid]) -> Vec<Uuid> {
    let mut unique = Vec::with_capacity(user_ids.len());
    for user_id in user_ids {
        if !unique.contains(user_id) {
            unique.push(*user_id);
        }
    }
    unique
}

fn resolve_backfill_display_name(
    display_name: Option<&str>,
    email: &str,
    user_id: Uuid,
) -> ProfileResult<String> {
    if let Some(display_name) = display_name
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        return ProfileService::normalize_display_name(display_name);
    }

    if let Some(display_name) = humanize_display_name(email_local_part(email)) {
        return ProfileService::normalize_display_name(&display_name);
    }

    ProfileService::normalize_display_name(&format!("User {}", short_user_suffix(user_id)))
}

fn email_local_part(email: &str) -> &str {
    email.split('@').next().unwrap_or(email)
}

fn humanize_display_name(seed: &str) -> Option<String> {
    let mut words = Vec::new();
    let mut current = String::new();

    for ch in seed.chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch.to_ascii_lowercase());
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        words.push(current);
    }

    if words.is_empty() {
        return None;
    }

    Some(
        words
            .into_iter()
            .map(|word| {
                let mut chars = word.chars();
                match chars.next() {
                    Some(first) => {
                        format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                    }
                    None => String::new(),
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}

fn slugify_handle_seed(seed: &str) -> Option<String> {
    let mut slug = String::new();
    let mut last_was_separator = false;

    for ch in seed.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if (ch == ' ' || ch == '-' || ch == '_' || !ch.is_ascii()) && !last_was_separator {
            if !slug.is_empty() {
                slug.push('-');
                last_was_separator = true;
            }
        }
    }

    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        None
    } else {
        Some(slug)
    }
}

fn build_handle_candidate(base: &str, suffix: usize) -> Option<String> {
    let suffix = if suffix == 0 {
        String::new()
    } else {
        format!("-{}", suffix + 1)
    };
    let max_base_len = MAX_HANDLE_LENGTH.checked_sub(suffix.len())?;
    if max_base_len < MIN_HANDLE_LENGTH {
        return None;
    }

    let trimmed_base = base
        .chars()
        .take(max_base_len)
        .collect::<String>()
        .trim_matches('-')
        .to_string();
    if trimmed_base.len() < MIN_HANDLE_LENGTH {
        return None;
    }

    ProfileService::normalize_handle(&format!("{trimmed_base}{suffix}")).ok()
}

fn short_user_suffix(user_id: Uuid) -> String {
    user_id.simple().to_string()[..8].to_string()
}

fn resolve_profile_translation_locale(
    profile: &entities::profile::Model,
    tenant_default_locale: Option<&str>,
) -> ProfileResult<String> {
    Ok(
        ProfileService::normalize_locale(profile.preferred_locale.as_deref())?
            .or(ProfileService::normalize_locale(tenant_default_locale)?)
            .unwrap_or_else(|| DEFAULT_PROFILE_LOCALE.to_string()),
    )
}

fn collect_batch_locales(
    profiles: &[entities::profile::Model],
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> ProfileResult<Vec<String>> {
    let mut locales = Vec::new();

    for locale in [
        ProfileService::normalize_locale(requested_locale)?,
        ProfileService::normalize_locale(tenant_default_locale)?,
    ]
    .into_iter()
    .flatten()
    {
        if !locales.contains(&locale) {
            locales.push(locale);
        }
    }

    for profile in profiles {
        if let Some(locale) = ProfileService::normalize_locale(profile.preferred_locale.as_deref())?
        {
            if !locales.contains(&locale) {
                locales.push(locale);
            }
        }
    }

    Ok(locales)
}

fn select_translation(
    translations: &HashMap<(Uuid, String), entities::profile_translation::Model>,
    profile: &entities::profile::Model,
    requested_locale: Option<&str>,
    tenant_default_locale: Option<&str>,
) -> ProfileResult<Option<entities::profile_translation::Model>> {
    for locale in ProfileService::locale_candidates(
        requested_locale,
        profile.preferred_locale.as_deref(),
        tenant_default_locale,
    )? {
        if let Some(translation) = translations.get(&(profile.user_id, locale)) {
            return Ok(Some(translation.clone()));
        }
    }

    Ok(None)
}

fn map_profile(
    profile: entities::profile::Model,
    translation: Option<entities::profile_translation::Model>,
    tags: Vec<String>,
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
        tags,
        avatar_media_id: profile.avatar_media_id,
        banner_media_id: profile.banner_media_id,
        preferred_locale: profile.preferred_locale,
        visibility: profile.visibility,
        status: profile.status,
    }
}

fn normalize_tag_names(tag_names: &[String]) -> Vec<String> {
    let mut normalized: Vec<String> = Vec::new();
    for tag_name in tag_names {
        let trimmed = tag_name.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !normalized
            .iter()
            .any(|existing| existing.eq_ignore_ascii_case(trimmed))
        {
            normalized.push(trimmed.to_string());
        }
    }
    normalized
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

    #[test]
    fn humanize_display_name_uses_email_local_part() {
        let display_name = super::resolve_backfill_display_name(
            None,
            "jane.doe-test@example.com",
            uuid::Uuid::nil(),
        )
        .unwrap();
        assert_eq!(display_name, "Jane Doe Test");
    }

    #[test]
    fn build_handle_candidate_adds_suffix_for_reserved_base() {
        assert_eq!(
            super::build_handle_candidate("admin", 1).as_deref(),
            Some("admin-2")
        );
    }

    #[test]
    fn slugify_handle_seed_discards_non_ascii_only_values() {
        assert_eq!(super::slugify_handle_seed("Привет мир"), None);
        assert_eq!(
            super::slugify_handle_seed("Jane Doe").as_deref(),
            Some("jane-doe")
        );
    }
}
