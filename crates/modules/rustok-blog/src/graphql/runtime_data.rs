use std::sync::Arc;

use rustok_api::graphql::GraphqlRuntimeInputs;
use rustok_comments_api::CommentsThreadPort;
use rustok_profiles_api::ProfileSummaryReader;
use rustok_profiles_api::ProfileSummaryReader;
use sea_orm::DatabaseConnection;

use crate::{CommentService, PublicCommentsSnapshotStore};

/// Manifest-attached Blog GraphQL runtime capabilities.
///
/// A host may publish transport-neutral Comments, public snapshot, and optional
/// profile-presentation capabilities through `HostRuntimeContext`. Missing owner
/// capabilities are represented as unavailable integrations; Blog never constructs
/// local provider fallbacks and keeps optional enrichment degraded rather than
/// changing the primary post read contract.
#[derive(Clone, Default)]
pub struct BlogGraphqlRuntimeData {
    comments_thread_port: Option<Arc<dyn CommentsThreadPort>>,
    public_comments_snapshot_store: Option<Arc<dyn PublicCommentsSnapshotStore>>,
    profile_summary_reader: Option<Arc<dyn ProfileSummaryReader>>,
    profile_summary_reader: Option<Arc<dyn ProfileSummaryReader>>,
}

pub fn attach_schema_data(inputs: &GraphqlRuntimeInputs) -> Result<BlogGraphqlRuntimeData, String> {
    Ok(BlogGraphqlRuntimeData {
        comments_thread_port: inputs.shared_get::<Arc<dyn CommentsThreadPort>>(),
        public_comments_snapshot_store: inputs.shared_get::<Arc<dyn PublicCommentsSnapshotStore>>(),
        profile_summary_reader: inputs.shared_get::<Arc<dyn ProfileSummaryReader>>(),
        profile_summary_reader: inputs.shared_get::<Arc<dyn ProfileSummaryReader>>(),
    })
}

impl BlogGraphqlRuntimeData {
    pub(crate) fn comment_service(
        &self,
        db: DatabaseConnection,
    ) -> CommentService {
        CommentService::from_optional_comments_thread_port(
            db,
            self.comments_thread_port.clone(),
        )
    }

    pub(crate) fn public_comments_snapshot_store(
        &self,
    ) -> Option<&Arc<dyn PublicCommentsSnapshotStore>> {
        self.public_comments_snapshot_store.as_ref()
    }

    pub(crate) fn profile_summary_reader(&self) -> Option<&Arc<dyn ProfileSummaryReader>> {
        self.profile_summary_reader.as_ref()
    }

    pub(crate) fn profile_summary_reader(&self) -> Option<&Arc<dyn ProfileSummaryReader>> {
        self.profile_summary_reader.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graphql_runtime_data_exposes_comments_port_selection() {
        let factory: fn(&GraphqlRuntimeInputs) -> Result<BlogGraphqlRuntimeData, String> =
            attach_schema_data;
        let selector: fn(
            &BlogGraphqlRuntimeData,
            DatabaseConnection,
        ) -> CommentService = BlogGraphqlRuntimeData::comment_service;
        let snapshot_selector: fn(
            &BlogGraphqlRuntimeData,
        ) -> Option<&Arc<dyn PublicCommentsSnapshotStore>> =
            BlogGraphqlRuntimeData::public_comments_snapshot_store;
        let profile_selector: fn(
            &BlogGraphqlRuntimeData,
        ) -> Option<&Arc<dyn ProfileSummaryReader>> =
            BlogGraphqlRuntimeData::profile_summary_reader;
        let _ = (factory, selector, snapshot_selector, profile_selector);
    }
}
