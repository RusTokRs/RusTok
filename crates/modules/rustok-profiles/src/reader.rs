use std::collections::HashMap;

use async_trait::async_trait;
use uuid::Uuid;

use crate::{ProfileRecord, ProfileResult, ProfileService, ProfileSummary};

#[async_trait]
pub trait ProfilesReader: Send + Sync {
    async fn find_profile_summary(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Option<ProfileSummary>>;

    async fn find_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<Uuid, ProfileSummary>>;

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
    async fn find_profile_summary(
        &self,
        tenant_id: Uuid,
        user_id: Uuid,
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<Option<ProfileSummary>> {
        match ProfileService::get_profile_summary(
            self,
            tenant_id,
            user_id,
            requested_locale,
            tenant_default_locale,
        )
        .await
        {
            Ok(summary) => Ok(Some(summary)),
            Err(crate::ProfileError::ProfileNotFound(_)) => Ok(None),
            Err(error) => Err(error),
        }
    }

    async fn find_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
    ) -> ProfileResult<HashMap<Uuid, ProfileSummary>> {
        self.find_profile_summaries_map(
            tenant_id,
            user_ids,
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
