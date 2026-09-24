use std::collections::HashMap;

use async_trait::async_trait;
use rustok_api::PortError;
use rustok_profiles_api::{
    ProfileSummary as ApiProfileSummary, ProfileSummaryAudience, ProfileSummaryReadError,
    ProfileSummaryReader,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::{
    ProfileAccessAudience, ProfileError, ProfilePrivacyDecision, ProfilePrivacyService,
    ProfileRecord, ProfileResult, ProfileService, ProfileSummary, ProfilesReader, entities,
};

/// Audience-bound profile presentation owner.
///
/// This is the `ProfilesReader` implementation intended for downstream
/// author/member/customer cards. It evaluates the canonical privacy matrix before
/// localized summaries are loaded, while the raw `ProfileService` reader remains
/// available for owner-internal workflows such as mention resolution and backfill.
#[derive(Clone)]
pub struct ProfilePresentationService {
    db: DatabaseConnection,
    audience: ProfileAccessAudience,
}

impl ProfilePresentationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::for_audience(db, ProfileAccessAudience::Anonymous)
    }

    pub fn for_audience(db: DatabaseConnection, audience: ProfileAccessAudience) -> Self {
        Self { db, audience }
    }

    pub async fn find_profile_summary(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Option<ProfileSummary>> {
        let mut summaries = self
            .find_profile_summaries(
                tenant_id,
                &[user_id],
                requested_locale,
                tenant_default_locale,
            )
            .await?;
        Ok(summaries.remove(&user_id))
    }

    pub async fn find_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<Uuid, ProfileSummary>> {
        if user_ids.is_empty() {
            return Ok(HashMap::new());
        }

        let decisions = ProfilePrivacyService::new(self.db.clone())
            .evaluate_access_batch(tenant_id, user_ids, self.audience)
            .await
            .map_err(map_privacy_error)?;
        let visible_user_ids = user_ids
            .iter()
            .copied()
            .filter(|user_id| decisions.get(user_id) == Some(&ProfilePrivacyDecision::Allow))
            .collect::<Vec<_>>();
        if visible_user_ids.is_empty() {
            return Ok(HashMap::new());
        }

        ProfileService::new(self.db.clone())
            .find_profile_summaries_map(
                tenant_id,
                &visible_user_ids,
                requested_locale,
                tenant_default_locale,
            )
            .await
    }

    pub async fn get_profile_by_handle(
        &self,
        tenant_id: Uuid,
        handle: &str,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        let normalized_handle = ProfileService::normalize_handle(handle)?;
        let profile = entities::profile::Entity::find()
            .filter(entities::profile::Column::TenantId.eq(tenant_id))
            .filter(entities::profile::Column::Handle.eq(normalized_handle.clone()))
            .one(&self.db)
            .await?
            .ok_or_else(|| ProfileError::ProfileByHandleNotFound(normalized_handle.clone()))?;
        let decision = ProfilePrivacyService::new(self.db.clone())
            .evaluate_access(tenant_id, profile.user_id, self.audience)
            .await
            .map_err(map_privacy_error)?;
        if decision != ProfilePrivacyDecision::Allow {
            return Err(ProfileError::ProfileByHandleNotFound(normalized_handle));
        }

        ProfileService::new(self.db.clone())
            .get_profile(
                tenant_id,
                profile.user_id,
                requested_locale,
                tenant_default_locale,
            )
            .await
    }

    pub async fn find_profile_records_by_handles(
        &self,
        tenant_id: Uuid,
        handles: &[String],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<String, ProfileRecord>> {
        let records = ProfileService::new(self.db.clone())
            .find_profile_records_by_handles(
                tenant_id,
                handles,
                requested_locale,
                tenant_default_locale,
            )
            .await?;
        if records.is_empty() {
            return Ok(HashMap::new());
        }

        let user_ids = records.values().map(|r| r.user_id).collect::<Vec<_>>();
        let decisions = ProfilePrivacyService::new(self.db.clone())
            .evaluate_access_batch(tenant_id, &user_ids, self.audience)
            .await
            .map_err(map_privacy_error)?;

        Ok(records
            .into_iter()
            .filter(|(_, r)| decisions.get(&r.user_id) == Some(&ProfilePrivacyDecision::Allow))
            .collect())
    }
}

#[async_trait]
impl ProfilesReader for ProfilePresentationService {
    async fn find_profile_summary(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Option<ProfileSummary>> {
        ProfilePresentationService::find_profile_summary(
            self,
            tenant_id,
            user_id,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }

    async fn find_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<Uuid, ProfileSummary>> {
        ProfilePresentationService::find_profile_summaries(
            self,
            tenant_id,
            user_ids,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }

    async fn find_profile_records_by_handles(
        &self,
        tenant_id: Uuid,
        handles: &[String],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<String, ProfileRecord>> {
        ProfilePresentationService::find_profile_records_by_handles(
            self,
            tenant_id,
            handles,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }

    async fn get_profile_by_handle(
        &self,
        tenant_id: Uuid,
        handle: &str,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        ProfilePresentationService::get_profile_by_handle(
            self,
            tenant_id,
            handle,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }
}

fn map_privacy_error(error: PortError) -> ProfileError {
    tracing::warn!(
        error_code = %error.code,
        retryable = error.retryable,
        "Profile presentation privacy evaluation failed"
    );
    ProfileError::PresentationUnavailable
}


#[async_trait]
impl ProfileSummaryReader for ProfilePresentationService {
    async fn find_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
        audience: ProfileSummaryAudience,
    ) -> Result<HashMap<Uuid, ApiProfileSummary>, ProfileSummaryReadError> {
        let reader = Self::for_audience(self.db.clone(), audience);
        reader
            .find_profile_summaries(
                tenant_id,
                user_ids,
                requested_locale,
                tenant_default_locale,
            )
            .await
            .map(|summaries| {
                summaries
                    .into_iter()
                    .map(|(user_id, summary)| (user_id, summary.into()))
                    .collect()
            })
            .map_err(|error| {
                tracing::warn!(
                    tenant_id = %tenant_id,
                    requested_count = user_ids.len(),
                    error = %error,
                    "Profile summary presentation provider is unavailable"
                );
                ProfileSummaryReadError::Unavailable
            })
    }
}
