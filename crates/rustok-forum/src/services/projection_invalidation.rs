use std::sync::Arc;

use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::error::ForumResult;

pub(crate) const FORUM_PROJECTION_SCOPE: &str = "forum";
pub(crate) const FORUM_CATEGORY_PROJECTION_TARGET: &str = "forum_category";
pub(crate) const FORUM_TOPIC_PROJECTION_TARGET: &str = "forum_topic";

pub(crate) async fn publish_forum_projection_scope_using_db_in_tx(
    db: &DatabaseConnection,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
) -> ForumResult<()> {
    publish_forum_projection_scope_in_tx(&event_bus(db), txn, tenant_id, actor_id).await
}

pub(crate) async fn publish_forum_category_projection_using_db_in_tx(
    db: &DatabaseConnection,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    category_id: Uuid,
) -> ForumResult<()> {
    publish_forum_category_projection_in_tx(
        &event_bus(db),
        txn,
        tenant_id,
        actor_id,
        category_id,
    )
    .await
}

pub(crate) async fn publish_forum_topic_projection_using_db_in_tx(
    db: &DatabaseConnection,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    topic_id: Uuid,
) -> ForumResult<()> {
    publish_forum_topic_projection_in_tx(&event_bus(db), txn, tenant_id, actor_id, topic_id).await
}

pub(crate) async fn publish_forum_projection_scope_in_tx(
    event_bus: &TransactionalEventBus,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
) -> ForumResult<()> {
    publish_projection_invalidation_in_tx(
        event_bus,
        txn,
        tenant_id,
        actor_id,
        FORUM_PROJECTION_SCOPE,
        None,
    )
    .await
}

pub(crate) async fn publish_forum_category_projection_in_tx(
    event_bus: &TransactionalEventBus,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    category_id: Uuid,
) -> ForumResult<()> {
    publish_projection_invalidation_in_tx(
        event_bus,
        txn,
        tenant_id,
        actor_id,
        FORUM_CATEGORY_PROJECTION_TARGET,
        Some(category_id),
    )
    .await
}

pub(crate) async fn publish_forum_topic_projection_in_tx(
    event_bus: &TransactionalEventBus,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    topic_id: Uuid,
) -> ForumResult<()> {
    publish_projection_invalidation_in_tx(
        event_bus,
        txn,
        tenant_id,
        actor_id,
        FORUM_TOPIC_PROJECTION_TARGET,
        Some(topic_id),
    )
    .await
}

async fn publish_projection_invalidation_in_tx(
    event_bus: &TransactionalEventBus,
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    target_type: &'static str,
    target_id: Option<Uuid>,
) -> ForumResult<()> {
    event_bus
        .publish_in_tx(
            txn,
            tenant_id,
            actor_id,
            DomainEvent::ReindexRequested {
                target_type: target_type.to_string(),
                target_id,
            },
        )
        .await?;
    Ok(())
}

fn event_bus(db: &DatabaseConnection) -> TransactionalEventBus {
    TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())))
}
