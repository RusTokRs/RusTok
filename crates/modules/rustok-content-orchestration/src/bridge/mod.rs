use async_trait::async_trait;
use rustok_content::{
    ContentOrchestrationBridge, ContentResult, DemotePostToTopicInput, DemotePostToTopicOutput,
    MergeTopicsInput, MergeTopicsOutput, PromoteTopicToPostInput, PromoteTopicToPostOutput,
    SplitTopicInput, SplitTopicOutput,
};
use rustok_taxonomy::TaxonomyService;
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

pub(crate) mod demote;
pub(crate) mod helpers;
pub(crate) mod merge;
pub(crate) mod promote;
pub(crate) mod split;
pub(crate) mod tags;

pub struct ServerContentOrchestrationBridge {
    taxonomy: TaxonomyService,
}

impl ServerContentOrchestrationBridge {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            taxonomy: TaxonomyService::new(db),
        }
    }
}

#[async_trait]
impl ContentOrchestrationBridge for ServerContentOrchestrationBridge {
    async fn promote_topic_to_post(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &PromoteTopicToPostInput,
    ) -> ContentResult<PromoteTopicToPostOutput> {
        promote::promote_topic_to_post(&self.taxonomy, txn, tenant_id, actor_id, input).await
    }

    async fn demote_post_to_topic(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &DemotePostToTopicInput,
    ) -> ContentResult<DemotePostToTopicOutput> {
        demote::demote_post_to_topic(&self.taxonomy, txn, tenant_id, actor_id, input).await
    }

    async fn split_topic(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &SplitTopicInput,
    ) -> ContentResult<SplitTopicOutput> {
        split::split_topic(&self.taxonomy, txn, tenant_id, actor_id, input).await
    }

    async fn merge_topics(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: &MergeTopicsInput,
    ) -> ContentResult<MergeTopicsOutput> {
        merge::merge_topics(txn, tenant_id, actor_id, input).await
    }
}
