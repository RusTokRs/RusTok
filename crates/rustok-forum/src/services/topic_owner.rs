include!("topic_owner_legacy.rs");

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
