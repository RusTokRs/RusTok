use std::collections::HashMap;

use async_graphql::dataloader::Loader;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::{ProfileAccessAudience, ProfilePresentationService, ProfileSummary};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProfileSummaryLoaderKey {
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub requested_locale: Option<String>,
    pub tenant_default_locale: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ProfileSummaryBatchKey {
    tenant_id: Uuid,
    requested_locale: Option<String>,
    tenant_default_locale: Option<String>,
}

impl From<&ProfileSummaryLoaderKey> for ProfileSummaryBatchKey {
    fn from(value: &ProfileSummaryLoaderKey) -> Self {
        Self {
            tenant_id: value.tenant_id,
            requested_locale: value.requested_locale.clone(),
            tenant_default_locale: value.tenant_default_locale.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ProfileSummaryLoader {
    db: DatabaseConnection,
    audience: ProfileAccessAudience,
}

impl ProfileSummaryLoader {
    /// Builds a fail-closed loader for public/anonymous presentation.
    ///
    /// Request-aware hosts should use [`Self::for_audience`] and attach the
    /// resulting DataLoader to request data so it overrides any schema-level
    /// fallback loader.
    pub fn new(db: DatabaseConnection) -> Self {
        Self::for_audience(db, ProfileAccessAudience::Anonymous)
    }

    pub fn for_audience(db: DatabaseConnection, audience: ProfileAccessAudience) -> Self {
        Self { db, audience }
    }
}

impl Loader<ProfileSummaryLoaderKey> for ProfileSummaryLoader {
    type Value = ProfileSummary;
    type Error = async_graphql::Error;

    fn load(
        &self,
        keys: &[ProfileSummaryLoaderKey],
    ) -> impl std::future::Future<
        Output = Result<HashMap<ProfileSummaryLoaderKey, Self::Value>, Self::Error>,
    > + Send {
        let db = self.db.clone();
        let audience = self.audience;
        let keys = keys.to_vec();

        async move {
            let mut grouped_user_ids: HashMap<ProfileSummaryBatchKey, Vec<Uuid>> = HashMap::new();
            for key in &keys {
                grouped_user_ids
                    .entry(ProfileSummaryBatchKey::from(key))
                    .or_default()
                    .push(key.user_id);
            }

            let presentation = ProfilePresentationService::for_audience(db, audience);
            let mut result = HashMap::with_capacity(keys.len());

            for (batch_key, user_ids) in grouped_user_ids {
                let summaries = presentation
                    .find_profile_summaries(
                        batch_key.tenant_id,
                        &user_ids,
                        batch_key.requested_locale.as_deref(),
                        batch_key.tenant_default_locale.as_deref(),
                    )
                    .await
                    .map_err(|error| async_graphql::Error::new(error.to_string()))?;

                for (user_id, summary) in summaries {
                    result.insert(
                        ProfileSummaryLoaderKey {
                            tenant_id: batch_key.tenant_id,
                            user_id,
                            requested_locale: batch_key.requested_locale.clone(),
                            tenant_default_locale: batch_key.tenant_default_locale.clone(),
                        },
                        summary,
                    );
                }
            }

            Ok(result)
        }
    }
}
