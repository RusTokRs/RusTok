use rustok_api::PortContext;
use rustok_core::SecurityContext;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::error::ForumResult;

use super::topic_audience_visibility::{
    ForumTopicAudienceViewer, ForumTopicAudienceVisibilityService,
};

/// Write-side audience gate for topic votes, reply votes, and topic subscriptions.
///
/// A write is allowed only when the caller can read the parent topic through the
/// owner audience contract: the base visibility floor, every inherited category layer,
/// and the topic-local layer. The gate returns `false` for both missing and denied
/// topics, so callers map the two cases to the same not-found error.
///
/// The gate does not require an open topic or a route channel. Those are read
/// contracts, not write contracts; see the crate README for the pending channel scope.
pub(crate) async fn topic_write_audience_allows(
    db: &DatabaseConnection,
    facts_port: Option<SharedForumAudienceFactsPort>,
    tenant_id: Uuid,
    topic_id: Uuid,
    security: &SecurityContext,
    context: PortContext,
) -> ForumResult<bool> {
    let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
    ForumTopicAudienceVisibilityService::new(db.clone(), facts_port)
        .is_topic_owner_visible(tenant_id, topic_id, &viewer)
        .await
}


/// Re-evaluates the canonical owner audience with transaction-scoped Forum
/// policy rows. The caller must first acquire the category-tree writer lock,
/// then keep the transaction open through the actual vote/subscription write.
pub(crate) async fn topic_write_audience_allows_in_tx(
    db: &DatabaseConnection,
    txn: &sea_orm::DatabaseTransaction,
    facts_port: Option<SharedForumAudienceFactsPort>,
    tenant_id: Uuid,
    topic_id: Uuid,
    security: &SecurityContext,
    context: PortContext,
) -> ForumResult<bool> {
    let viewer = ForumTopicAudienceViewer::authenticated(security.clone(), context)?;
    ForumTopicAudienceVisibilityService::new(db.clone(), facts_port)
        .is_topic_owner_visible_in_tx(txn, tenant_id, topic_id, &viewer)
        .await
}
