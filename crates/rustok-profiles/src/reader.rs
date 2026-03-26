use async_trait::async_trait;
use uuid::Uuid;

use crate::{ProfileRecord, ProfileResult, ProfileService, ProfileSummary};

#[async_trait]
pub trait ProfilesReader: Send + Sync {
    async fn get_profile_summary(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileSummary>;

    async fn get_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Vec<ProfileSummary>>;

    async fn get_profile_by_handle(
        &self,
        tenant_id: Uuid,
        handle: &str,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord>;
}

#[async_trait]
impl ProfilesReader for ProfileService {
    async fn get_profile_summary(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileSummary> {
        ProfileService::get_profile_summary(
            self,
            tenant_id,
            user_id,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }

    async fn get_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Vec<ProfileSummary>> {
        let mut profiles = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            profiles.push(
                ProfileService::get_profile_summary(
                    self,
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

    async fn get_profile_by_handle(
        &self,
        tenant_id: Uuid,
        handle: &str,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<ProfileRecord> {
        ProfileService::get_profile_by_handle(
            self,
            tenant_id,
            handle,
            requested_locale,
            tenant_default_locale,
        )
        .await
    }
}
