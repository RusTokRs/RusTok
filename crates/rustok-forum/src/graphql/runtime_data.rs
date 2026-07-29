use rustok_api::graphql::GraphqlRuntimeInputs;
use rustok_outbox::TransactionalEventBus;
use sea_orm::DatabaseConnection;

use crate::{ModerationService, ReplyService, SharedForumAudienceFactsPort, TopicService};

/// Manifest-attached Forum GraphQL runtime capabilities.
///
/// The optional facts port is published by the host runtime extension registry.
/// Its absence is preserved so locally decidable create and moderation policies
/// continue to work while trust, Channel, or Groups facts fail closed in owners.
#[derive(Clone, Default)]
pub struct ForumGraphqlRuntimeData {
    audience_facts: Option<SharedForumAudienceFactsPort>,
}

pub fn attach_schema_data(
    inputs: &GraphqlRuntimeInputs,
) -> Result<ForumGraphqlRuntimeData, String> {
    Ok(ForumGraphqlRuntimeData {
        audience_facts: inputs.shared_get::<SharedForumAudienceFactsPort>(),
    })
}

impl ForumGraphqlRuntimeData {
    pub(crate) fn topic_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> TopicService {
        match self.audience_facts.clone() {
            Some(facts) => TopicService::with_audience_facts(db, event_bus, facts),
            None => TopicService::new(db, event_bus),
        }
    }

    pub(crate) fn reply_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> ReplyService {
        match self.audience_facts.clone() {
            Some(facts) => ReplyService::with_audience_facts(db, event_bus, facts),
            None => ReplyService::new(db, event_bus),
        }
    }

    pub(crate) fn moderation_service(
        &self,
        db: DatabaseConnection,
        event_bus: TransactionalEventBus,
    ) -> ModerationService {
        match self.audience_facts.clone() {
            Some(facts) => ModerationService::with_audience_facts(db, event_bus, facts),
            None => ModerationService::new(db, event_bus),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use async_trait::async_trait;
    use rustok_api::{HostRuntimeContext, PortContext, PortError};
    use sea_orm::Database;

    use crate::{ForumAudienceFacts, ForumAudienceFactsPort, ForumAudienceFactsRequest};

    use super::*;

    struct StaticFactsPort;

    #[async_trait]
    impl ForumAudienceFactsPort for StaticFactsPort {
        async fn resolve_forum_audience_facts(
            &self,
            _context: PortContext,
            request: ForumAudienceFactsRequest,
        ) -> Result<ForumAudienceFacts, PortError> {
            Ok(ForumAudienceFacts {
                tenant_id: request.tenant_id,
                user_id: request.user_id,
                ..ForumAudienceFacts::default()
            })
        }
    }

    #[tokio::test]
    async fn schema_factory_consumes_host_published_audience_facts_without_db_discovery() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("GraphQL runtime test database should connect");
        let facts: SharedForumAudienceFactsPort = Arc::new(StaticFactsPort);
        let inputs =
            GraphqlRuntimeInputs::new(HostRuntimeContext::new(db).with_shared_value(facts.clone()));

        let runtime =
            attach_schema_data(&inputs).expect("Forum GraphQL runtime should materialize");
        assert!(runtime.audience_facts.is_some());
    }

    #[tokio::test]
    async fn schema_factory_preserves_optional_provider_absence() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("GraphQL runtime test database should connect");
        let inputs = GraphqlRuntimeInputs::new(HostRuntimeContext::new(db));

        let runtime =
            attach_schema_data(&inputs).expect("Forum GraphQL runtime should materialize");
        assert!(runtime.audience_facts.is_none());
    }
}
