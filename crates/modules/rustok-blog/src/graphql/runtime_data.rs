use std::sync::Arc;

use rustok_api::graphql::GraphqlRuntimeInputs;
use rustok_comments::CommentsThreadPort;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{CommentService, PublicCommentsSnapshotStore};

/// Manifest-attached Blog GraphQL runtime capabilities.
///
/// A host may publish transport-neutral Comments and public snapshot capabilities
/// through `HostRuntimeContext`. Their absence selects the canonical in-process
/// Comments adapter and empty degraded snapshot behavior.
#[derive(Clone, Default)]
pub struct BlogGraphqlRuntimeData {
    comments_thread_port: Option<Arc<dyn CommentsThreadPort>>,
    public_comments_snapshot_store: Option<Arc<dyn PublicCommentsSnapshotStore>>,
}

pub fn attach_schema_data(inputs: &GraphqlRuntimeInputs) -> Result<BlogGraphqlRuntimeData, String> {
    Ok(BlogGraphqlRuntimeData {
        comments_thread_port: inputs.shared_get::<Arc<dyn CommentsThreadPort>>(),
        public_comments_snapshot_store: inputs.shared_get::<Arc<dyn PublicCommentsSnapshotStore>>(),
    })
}

impl BlogGraphqlRuntimeData {
    pub(crate) fn comment_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> CommentService {
        match self.comments_thread_port.clone() {
            Some(comments_thread_port) => {
                CommentService::with_comments_thread_port(db, comments_thread_port)
            }
            None => CommentService::new(db, event_bus),
        }
    }

    pub(crate) fn public_comments_snapshot_store(
        &self,
    ) -> Option<&Arc<dyn PublicCommentsSnapshotStore>> {
        self.public_comments_snapshot_store.as_ref()
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
            TransactionalEventBus,
        ) -> CommentService = BlogGraphqlRuntimeData::comment_service;
        let snapshot_selector: fn(
            &BlogGraphqlRuntimeData,
        ) -> Option<&Arc<dyn PublicCommentsSnapshotStore>> =
            BlogGraphqlRuntimeData::public_comments_snapshot_store;
        let _ = (factory, selector, snapshot_selector);
    }
}
