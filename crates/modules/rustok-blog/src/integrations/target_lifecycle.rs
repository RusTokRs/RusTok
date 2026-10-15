//! Blog-side reaction to target deletion.
//!
//! `blog_post_routes` holds the retired Blog routes that redirect to a post. The
//! canonical delete path removes them inside its own transaction, but a post can
//! also disappear through another owner's path (content orchestration, an admin
//! purge, or an owner-side cascade). A redirect whose target is gone must not keep
//! a user-facing route alive, so Blog consumes `TargetDeleted` and removes the
//! matching rows idempotently: the delete is unconditional, and a repeated delivery
//! finds no rows and publishes no event.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::Error as CoreError;
use rustok_core::events::{EventEnvelope, EventHandler, HandlerResult};
use rustok_events::DomainEvent;
use rustok_outbox::{OutboxTransport, TransactionalEventBus};
use sea_orm::{DatabaseConnection, TransactionTrait};

use crate::error::BlogError;
use crate::services::{BLOG_POST_TARGET_TYPE, BlogPostRouteOwner};

pub(crate) struct BlogTargetDeletionRouteHandler {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl BlogTargetDeletionRouteHandler {
    /// The listener owns its transactional bus over the same database, exactly like
    /// the Blog comment projection handler: purge events are written to the outbox
    /// inside the deletion transaction.
    pub(crate) fn new(db: DatabaseConnection) -> Self {
        let event_bus = TransactionalEventBus::new(Arc::new(OutboxTransport::new(db.clone())));
        Self { db, event_bus }
    }
}

#[async_trait]
impl EventHandler for BlogTargetDeletionRouteHandler {
    fn name(&self) -> &'static str {
        "blog_target_deletion_routes"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(
            event,
            DomainEvent::TargetDeleted { target_type, .. }
                if target_type == BLOG_POST_TARGET_TYPE
        )
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        let DomainEvent::TargetDeleted {
            target_type,
            target_id,
        } = &envelope.event
        else {
            return Ok(());
        };
        if target_type != BLOG_POST_TARGET_TYPE {
            return Ok(());
        }

        let txn = self.db.begin().await?;
        let owner = BlogPostRouteOwner::new(self.event_bus.clone());
        // The delivery removes the routes and reports them through `UrlAliasPurged`,
        // so SEO drops its redirect instead of keeping a route that resolves nowhere.
        let removed = owner
            .remove_target_routes_in_tx(&txn, envelope.tenant_id, None, *target_id)
            .await
            .map_err(handler_error)?;
        txn.commit().await?;

        if removed > 0 {
            tracing::info!(
                tenant_id = %envelope.tenant_id,
                target_id = %target_id,
                removed,
                "Blog removed routes whose post was deleted outside the Blog delete path"
            );
        }
        Ok(())
    }
}

/// Preserves the error class the dispatcher logs and retries on: a database failure
/// stays a database failure, a Blog core dependency failure stays itself, and any
/// other Blog error is reported as an internal handler failure with its detail.
fn handler_error(error: BlogError) -> CoreError {
    match error {
        BlogError::Database(db_err) => CoreError::Database(db_err),
        BlogError::Core(core) => core,
        other => CoreError::Internal(other.to_string()),
    }
}
