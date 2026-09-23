use std::collections::HashMap;

use async_trait::async_trait;
use uuid::Uuid;

use crate::{ProfileSummary, ProfileSummaryAudience, ProfileSummaryReadResult};

#[async_trait]
pub trait ProfileSummaryReader: Send + Sync {
    async fn find_profile_summaries(
        &self,
        tenant_id: Uuid,
        user_ids: &[Uuid],
        requested_locale: Option<&str>,
        tenant_default_locale: Option<&str>,
        audience: ProfileSummaryAudience,
    ) -> ProfileSummaryReadResult<HashMap<Uuid, ProfileSummary>>;
}
