use super::{category_audience, category_visibility, topic_audience, topic_audience_lock};

#[path = "topic_route_tombstone_visibility.rs"]
pub mod route_tombstone_visibility;

use crate::dto::{CreateTopicCommandInput, UpdateTopicCommandInput};

impl TopicService {
    #[instrument(skip(self, security, input))]
    pub async fn create_command(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        self.inner
            .create_with_inline_relations(tenant_id, security, input)
            .await
    }

    pub(crate) async fn create_with_audience_authorization(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        context: Option<PortContext>,
        input: CreateTopicCommandInput,
        create_audience: &ForumTopicCreateAudienceAuthorizationService,
    ) -> ForumResult<TopicResponse> {
        self.inner
            .create_with_audience_authorization(
                tenant_id,
                security,
                context,
                input,
                create_audience,
            )
            .await
    }

    #[instrument(skip(self, security, input))]
    pub async fn update_command(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        input: UpdateTopicCommandInput,
    ) -> ForumResult<TopicResponse> {
        self.inner
            .update_with_inline_relations(tenant_id, topic_id, security, input)
            .await
    }
}
