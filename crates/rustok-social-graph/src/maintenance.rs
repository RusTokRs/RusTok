use std::time::Instant;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError};
use rustok_outbox::TransactionalEventBus;
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    TransactionTrait,
};
use uuid::Uuid;

use crate::entities::relation;
use crate::error::SocialGraphError;
use crate::external_events::event_for_relation;
use crate::ports::{
    MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH, MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH,
    SocialGraphReceiptCleanupCommand, SocialGraphReceiptCleanupResult,
    SocialGraphReceiptMaintenancePort, SocialGraphRelationEventMaintenancePort,
    SocialGraphRelationEventReplayCommand, SocialGraphRelationEventReplayResult, map_owner_error,
    parse_tenant_id,
};
use crate::receipts;

const RECEIPT_CLEANUP_OPERATION: &str = "social_graph.receipt_cleanup";
const RELATION_EVENT_REPLAY_OPERATION: &str = "social_graph.relation_event_replay";

#[derive(Clone, Debug)]
pub struct SocialGraphReceiptMaintenanceService {
    db: DatabaseConnection,
}

impl SocialGraphReceiptMaintenanceService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[derive(Clone)]
pub struct SocialGraphRelationEventMaintenanceService {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl SocialGraphRelationEventMaintenanceService {
    pub fn new(db: DatabaseConnection, event_bus: TransactionalEventBus) -> Self {
        Self { db, event_bus }
    }
}

#[async_trait]
impl SocialGraphReceiptMaintenancePort for SocialGraphReceiptMaintenanceService {
    async fn cleanup_completed_receipts(
        &self,
        context: PortContext,
        command: SocialGraphReceiptCleanupCommand,
    ) -> Result<SocialGraphReceiptCleanupResult, PortError> {
        let tenant_id = parse_tenant_id(&context)?;
        let started = Instant::now();

        if let Err(error) = context.require_policy(PortCallPolicy::write()) {
            log_cleanup_failure(tenant_id, command, started, &error);
            return Err(error);
        }
        if matches!(&context.actor.kind, PortActorKind::User) {
            let error = PortError::forbidden(
                "social_graph.receipt_cleanup_forbidden",
                "social graph receipt cleanup requires a service or system actor",
            );
            log_cleanup_failure(tenant_id, command, started, &error);
            return Err(error);
        }
        if command.limit == 0 || command.limit > MAX_SOCIAL_GRAPH_RECEIPT_CLEANUP_BATCH {
            let error = PortError::validation(
                "social_graph.receipt_cleanup_limit_invalid",
                "social graph receipt cleanup limit must be between 1 and 1000",
            );
            log_cleanup_failure(tenant_id, command, started, &error);
            return Err(error);
        }
        let completed_before =
            match DateTime::<Utc>::from_timestamp(command.completed_before_unix_seconds, 0) {
                Some(value) => value,
                None => {
                    let error = PortError::validation(
                        "social_graph.receipt_cleanup_cutoff_invalid",
                        "social graph receipt cleanup cutoff must be a valid Unix timestamp",
                    );
                    log_cleanup_failure(tenant_id, command, started, &error);
                    return Err(error);
                }
            };
        if completed_before >= Utc::now() {
            let error = PortError::validation(
                "social_graph.receipt_cleanup_cutoff_future",
                "social graph receipt cleanup cutoff must be in the past",
            );
            log_cleanup_failure(tenant_id, command, started, &error);
            return Err(error);
        }

        let result = receipts::cleanup_completed(
            &self.db,
            tenant_id,
            completed_before.fixed_offset(),
            u64::from(command.limit),
            command.dry_run,
        )
        .await;
        let duration_ms = duration_ms(started);

        match result {
            Ok((matched_receipts, deleted_receipts, oldest_retained_completed_at_unix_seconds)) => {
                tracing::info!(
                    target: "rustok_social_graph::operations",
                    operation = RECEIPT_CLEANUP_OPERATION,
                    tenant_id = %tenant_id,
                    dry_run = command.dry_run,
                    limit = command.limit,
                    matched_receipts,
                    deleted_receipts,
                    oldest_retained_completed_at_unix_seconds = ?oldest_retained_completed_at_unix_seconds,
                    duration_ms,
                    outcome = "success",
                    "social graph receipt cleanup completed"
                );
                Ok(SocialGraphReceiptCleanupResult {
                    matched_receipts,
                    deleted_receipts,
                    oldest_retained_completed_at_unix_seconds,
                })
            }
            Err(error) => {
                let error = map_owner_error(error);
                log_cleanup_failure(tenant_id, command, started, &error);
                Err(error)
            }
        }
    }
}

#[async_trait]
impl SocialGraphRelationEventMaintenancePort for SocialGraphRelationEventMaintenanceService {
    async fn replay_relation_state_events(
        &self,
        context: PortContext,
        command: SocialGraphRelationEventReplayCommand,
    ) -> Result<SocialGraphRelationEventReplayResult, PortError> {
        let tenant_id = parse_tenant_id(&context)?;
        let started = Instant::now();

        if let Err(error) = context.require_policy(PortCallPolicy::event_replay()) {
            log_relation_event_replay_failure(tenant_id, command, started, &error);
            return Err(error);
        }
        if matches!(&context.actor.kind, PortActorKind::User) {
            let error = PortError::forbidden(
                "social_graph.relation_event_replay_forbidden",
                "social graph relation event replay requires a service or system actor",
            );
            log_relation_event_replay_failure(tenant_id, command, started, &error);
            return Err(error);
        }
        if command.limit == 0 || command.limit > MAX_SOCIAL_GRAPH_RELATION_EVENT_REPLAY_BATCH {
            let error = PortError::validation(
                "social_graph.relation_event_replay_limit_invalid",
                "social graph relation event replay limit must be between 1 and 1000",
            );
            log_relation_event_replay_failure(tenant_id, command, started, &error);
            return Err(error);
        }

        let transaction = self
            .db
            .begin()
            .await
            .map_err(|error| map_owner_error(SocialGraphError::from(error)))?;
        let mut query = relation::Entity::find()
            .filter(relation::Column::TenantId.eq(tenant_id))
            .order_by_asc(relation::Column::Id)
            .limit(u64::from(command.limit));
        if let Some(after_relation_id) = command.after_relation_id {
            query = query.filter(relation::Column::Id.gt(after_relation_id));
        }
        let relations = match query.all(&transaction).await {
            Ok(relations) => relations,
            Err(error) => {
                let port_error = map_owner_error(SocialGraphError::from(error));
                if let Err(rollback_error) = transaction.rollback().await {
                    let rollback_error = map_owner_error(SocialGraphError::from(rollback_error));
                    log_relation_event_replay_failure(tenant_id, command, started, &rollback_error);
                    return Err(rollback_error);
                }
                log_relation_event_replay_failure(tenant_id, command, started, &port_error);
                return Err(port_error);
            }
        };

        let selected_relations = relations.len() as u64;
        let next_after_relation_id = relations.last().map(|relation| relation.id);
        let actor_id = Uuid::parse_str(&context.actor.id).ok();
        let mut published_events = 0_u64;

        if !command.dry_run {
            for relation in &relations {
                if self
                    .event_bus
                    .publish_contract_in_tx(
                        &transaction,
                        tenant_id,
                        actor_id,
                        event_for_relation(relation),
                    )
                    .await
                    .is_err()
                {
                    let port_error = map_owner_error(SocialGraphError::EventPublicationUnavailable);
                    if let Err(rollback_error) = transaction.rollback().await {
                        let rollback_error =
                            map_owner_error(SocialGraphError::from(rollback_error));
                        log_relation_event_replay_failure(
                            tenant_id,
                            command,
                            started,
                            &rollback_error,
                        );
                        return Err(rollback_error);
                    }
                    log_relation_event_replay_failure(tenant_id, command, started, &port_error);
                    return Err(port_error);
                }
                published_events = published_events.saturating_add(1);
            }
        }

        if let Err(error) = transaction.commit().await {
            let error = map_owner_error(SocialGraphError::from(error));
            log_relation_event_replay_failure(tenant_id, command, started, &error);
            return Err(error);
        }

        let duration_ms = duration_ms(started);
        tracing::info!(
            target: "rustok_social_graph::operations",
            operation = RELATION_EVENT_REPLAY_OPERATION,
            tenant_id = %tenant_id,
            dry_run = command.dry_run,
            limit = command.limit,
            cursor_present = command.after_relation_id.is_some(),
            selected_relations,
            published_events,
            has_next_cursor = next_after_relation_id.is_some(),
            duration_ms,
            outcome = "success",
            "social graph relation event replay completed"
        );

        Ok(SocialGraphRelationEventReplayResult {
            selected_relations,
            published_events,
            next_after_relation_id,
        })
    }
}

fn log_cleanup_failure(
    tenant_id: Uuid,
    command: SocialGraphReceiptCleanupCommand,
    started: Instant,
    error: &PortError,
) {
    let duration_ms = duration_ms(started);
    tracing::warn!(
        target: "rustok_social_graph::operations",
        operation = RECEIPT_CLEANUP_OPERATION,
        tenant_id = %tenant_id,
        dry_run = command.dry_run,
        limit = command.limit,
        duration_ms,
        outcome = "failure",
        error_code = %error.code,
        retryable = error.retryable,
        "social graph receipt cleanup failed"
    );
}

fn log_relation_event_replay_failure(
    tenant_id: Uuid,
    command: SocialGraphRelationEventReplayCommand,
    started: Instant,
    error: &PortError,
) {
    let duration_ms = duration_ms(started);
    tracing::warn!(
        target: "rustok_social_graph::operations",
        operation = RELATION_EVENT_REPLAY_OPERATION,
        tenant_id = %tenant_id,
        dry_run = command.dry_run,
        limit = command.limit,
        cursor_present = command.after_relation_id.is_some(),
        duration_ms,
        outcome = "failure",
        error_code = %error.code,
        retryable = error.retryable,
        "social graph relation event replay failed"
    );
}

fn duration_ms(started: Instant) -> u64 {
    started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64
}
