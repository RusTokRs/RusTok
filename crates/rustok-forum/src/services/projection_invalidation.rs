use rustok_events::{DomainEvent, ValidateEvent};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction};
use uuid::Uuid;

use crate::error::{ForumError, ForumResult};

pub(crate) const FORUM_PROJECTION_SCOPE: &str = "forum";
pub(crate) const FORUM_CATEGORY_PROJECTION_TARGET: &str = "forum_category";
pub(crate) const FORUM_TOPIC_PROJECTION_TARGET: &str = "forum_topic";

pub(crate) async fn publish_forum_projection_scope_direct_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
) -> ForumResult<()> {
    write_projection_invalidation_in_tx(
        txn,
        tenant_id,
        actor_id,
        FORUM_PROJECTION_SCOPE,
        None,
    )
    .await
}

pub(crate) async fn publish_forum_category_projection_direct_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    category_id: Uuid,
) -> ForumResult<()> {
    write_projection_invalidation_in_tx(
        txn,
        tenant_id,
        actor_id,
        FORUM_CATEGORY_PROJECTION_TARGET,
        Some(category_id),
    )
    .await
}

pub(crate) async fn publish_forum_topic_projection_direct_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    topic_id: Uuid,
) -> ForumResult<()> {
    write_projection_invalidation_in_tx(
        txn,
        tenant_id,
        actor_id,
        FORUM_TOPIC_PROJECTION_TARGET,
        Some(topic_id),
    )
    .await
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

async fn write_projection_invalidation_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    target_type: &'static str,
    target_id: Option<Uuid>,
) -> ForumResult<()> {
    let event = DomainEvent::ReindexRequested {
        target_type: target_type.to_string(),
        target_id,
    };

    // The Search-owned Forum projector is PostgreSQL-only. SQLite and any
    // other non-PostgreSQL backend are domain-test/unsupported projection
    // environments, so keep root validation without requiring an outbox table
    // that has no matching Search consumer.
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        event.validate().map_err(|error| {
            ForumError::Validation(format!("Forum projection invalidation failed: {error}"))
        })?;
        return Ok(());
    }

    TransactionalEventBus::publish_root_in_tx(txn, tenant_id, actor_id, event).await?;
    Ok(())
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
