use std::sync::Arc;

use rustok_api::graphql::GraphqlRuntimeInputs;
use rustok_comments::CommentsThreadPort;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::CommentService;

/// Manifest-attached Blog GraphQL runtime capabilities.
///
/// A host may publish a transport-neutral Comments port through
/// `HostRuntimeContext`. Its absence preserves the existing in-process Blog
/// compatibility profile.
#[derive(Clone, Default)]
pub struct BlogGraphqlRuntimeData {
    comments_thread_port: Option<Arc<dyn CommentsThreadPort>>,
}

pub fn attach_schema_data(
    inputs: &GraphqlRuntimeInputs,
) -> Result<BlogGraphqlRuntimeData, String> {
    Ok(BlogGraphqlRuntimeData {
        comments_thread_port: inputs.shared_get::<Arc<dyn CommentsThreadPort>>(),
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
        let _ = (factory, selector);
    }
}
