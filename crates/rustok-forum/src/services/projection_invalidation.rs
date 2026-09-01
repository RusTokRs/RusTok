use rustok_events::{DomainEvent, ForumSearchProjectionEvent, ValidateEvent};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, DbBackend, Statement};
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
    write_projection_invalidation_in_tx(txn, tenant_id, actor_id, FORUM_PROJECTION_SCOPE, None)
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
    let root_event = projection_invalidation_event(target_type, target_id);

    // The Search-owned Forum projector is PostgreSQL-only. SQLite and any
    // other non-PostgreSQL backend are domain-test/unsupported projection
    // environments, so keep root validation without requiring an outbox or
    // owner-revision ledger that has no matching Search consumer.
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        root_event.validate().map_err(|error| {
            ForumError::Validation(format!("Forum projection invalidation failed: {error}"))
        })?;
        return Ok(());
    }

    let revision = allocate_projection_revision_in_tx(txn, tenant_id).await?;
    let root_event_id = TransactionalEventBus::publish_root_in_tx_with_envelope_id(
        txn, tenant_id, actor_id, root_event,
    )
    .await?;
    TransactionalEventBus::publish_contract_direct_in_tx_with_causation_and_envelope_id(
        txn,
        tenant_id,
        actor_id,
        root_event_id,
        projection_invalidation_contract(revision, target_type, target_id),
    )
    .await?;
    record_projection_revision_in_tx(
        txn,
        tenant_id,
        revision,
        root_event_id,
        target_type,
        target_id,
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
    let root_event = projection_invalidation_event(target_type, target_id);
    if txn.get_database_backend() != DatabaseBackend::Postgres {
        event_bus
            .publish_in_tx(txn, tenant_id, actor_id, root_event)
            .await?;
        return Ok(());
    }

    let revision = allocate_projection_revision_in_tx(txn, tenant_id).await?;
    let root_event_id = event_bus
        .publish_in_tx_with_envelope_id(txn, tenant_id, actor_id, root_event)
        .await?;
    event_bus
        .publish_contract_in_tx_with_causation(
            txn,
            tenant_id,
            actor_id,
            root_event_id,
            projection_invalidation_contract(revision, target_type, target_id),
        )
        .await?;
    record_projection_revision_in_tx(
        txn,
        tenant_id,
        revision,
        root_event_id,
        target_type,
        target_id,
    )
    .await
}

fn projection_invalidation_event(
    target_type: &'static str,
    target_id: Option<Uuid>,
) -> DomainEvent {
    DomainEvent::ReindexRequested {
        target_type: target_type.to_string(),
        target_id,
    }
}

fn projection_invalidation_contract(
    owner_revision: i64,
    target_type: &'static str,
    target_id: Option<Uuid>,
) -> ForumSearchProjectionEvent {
    ForumSearchProjectionEvent::InvalidationIssued {
        owner_revision,
        target_type: target_type.to_string(),
        target_id,
    }
}

async fn allocate_projection_revision_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
) -> ForumResult<i64> {
    let row = txn
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
            INSERT INTO forum_projection_revision_counters (
                tenant_id, revision, updated_at
            ) VALUES ($1, 1, CURRENT_TIMESTAMP)
            ON CONFLICT (tenant_id)
            DO UPDATE SET
                revision = forum_projection_revision_counters.revision + 1,
                updated_at = CURRENT_TIMESTAMP
            RETURNING revision
            "#,
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| {
            ForumError::Validation(
                "Forum projection revision allocation returned no row".to_string(),
            )
        })?;
    let revision: i64 = row.try_get("", "revision")?;
    if revision <= 0 {
        return Err(ForumError::Validation(
            "Forum projection revision must be positive".to_string(),
        ));
    }
    Ok(revision)
}

async fn record_projection_revision_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    revision: i64,
    event_id: Uuid,
    target_type: &'static str,
    target_id: Option<Uuid>,
) -> ForumResult<()> {
    txn.execute_raw(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
        INSERT INTO forum_projection_revision_ledger (
            tenant_id, revision, event_id, target_type, target_id, created_at
        ) VALUES ($1, $2, $3, $4, $5, CURRENT_TIMESTAMP)
        "#,
        vec![
            tenant_id.into(),
            revision.into(),
            event_id.into(),
            target_type.to_string().into(),
            target_id.into(),
        ],
    ))
    .await?;
    Ok(())
}
