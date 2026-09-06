//! Bounded item-specific drain authorization for predecessor-incompatible queued work.
//!
//! Provides explicit, auditable draining (cancellation) of pending event and schedule
//! deliveries for an installation undergoing predecessor-incompatible migration,
//! decommissioning, or emergency shutdown without creating synthetic work or network traffic.

use sea_orm::{ConnectionTrait, DatabaseConnection, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ModuleCommandContext,
    data::{configure_tenant_scope, now_expression, placeholder, uuid_value},
};

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactQueueDrainError {
    #[error("Command context tenant does not match request tenant")]
    TenantMismatch,
    #[error("Drain reason must not be empty")]
    EmptyReason,
    #[error("Storage error: {0}")]
    Storage(String),
}

fn storage_error<E: std::fmt::Display>(e: E) -> ArtifactQueueDrainError {
    ArtifactQueueDrainError::Storage(e.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactQueueDrainRequest {
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub limit: u32,
    pub context: ModuleCommandContext,
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactQueueDrainReceipt {
    pub tenant_id: Uuid,
    pub installation_id: Uuid,
    pub drained_events: u64,
    pub drained_schedules: u64,
    pub remaining_pending: u64,
}

pub struct ArtifactQueueDrainService {
    db: DatabaseConnection,
}

impl ArtifactQueueDrainService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    /// Drains a bounded number of pending event and schedule deliveries for the given installation.
    ///
    /// Marked deliveries are terminated with `predecessor_incompatible_drain` and timestamped
    /// without dispatching any handler or generating external traffic.
    pub async fn drain_incompatible_work(
        &self,
        request: ArtifactQueueDrainRequest,
    ) -> Result<ArtifactQueueDrainReceipt, ArtifactQueueDrainError> {
        if request.context.tenant_id != Some(request.tenant_id) {
            return Err(ArtifactQueueDrainError::TenantMismatch);
        }
        if request.reason.trim().is_empty() {
            return Err(ArtifactQueueDrainError::EmptyReason);
        }

        let limit = request.limit.clamp(1, 500);
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.tenant_id)
            .await
            .map_err(storage_error)?;

        let backend = transaction.get_database_backend();

        // 1. Drain pending event deliveries
        let drained_events_result = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_event_deliveries \
                     SET status = 'dead_letter', dead_lettered_at = {}, \
                         last_error_code = 'predecessor_incompatible_drain', \
                         claimed_by = NULL, claimed_until = NULL \
                     WHERE delivery_id IN ( \
                         SELECT delivery_id FROM module_artifact_event_deliveries \
                         WHERE tenant_id = {} AND installation_id = {} AND status = 'pending' \
                         LIMIT {} \
                     )",
                    now_expression(backend),
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    limit,
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        let drained_events = drained_events_result.rows_affected();

        // 2. Drain pending schedule deliveries
        let drained_schedules_result = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_schedule_deliveries \
                     SET status = 'cancelled', cancelled_at = {}, \
                         last_error_code = 'predecessor_incompatible_drain', \
                         claimed_by = NULL, claimed_until = NULL \
                     WHERE delivery_id IN ( \
                         SELECT delivery_id FROM module_artifact_schedule_deliveries \
                         WHERE tenant_id = {} AND installation_id = {} AND status = 'pending' \
                         LIMIT {} \
                     )",
                    now_expression(backend),
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    limit,
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;
        let drained_schedules = drained_schedules_result.rows_affected();

        // 3. Query remaining pending deliveries
        let remaining_events_row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT COUNT(*) AS count FROM module_artifact_event_deliveries \
                     WHERE tenant_id = {} AND installation_id = {} AND status = 'pending'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;

        let remaining_schedules_row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT COUNT(*) AS count FROM module_artifact_schedule_deliveries \
                     WHERE tenant_id = {} AND installation_id = {} AND status = 'pending'",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                ),
                vec![
                    uuid_value(request.tenant_id, backend),
                    uuid_value(request.installation_id, backend),
                ],
            ))
            .await
            .map_err(storage_error)?;

        let remaining_events: i64 = remaining_events_row
            .and_then(|r| r.try_get("", "count").ok())
            .unwrap_or(0);
        let remaining_schedules: i64 = remaining_schedules_row
            .and_then(|r| r.try_get("", "count").ok())
            .unwrap_or(0);
        let remaining_pending = (remaining_events + remaining_schedules) as u64;

        transaction.commit().await.map_err(storage_error)?;

        Ok(ArtifactQueueDrainReceipt {
            tenant_id: request.tenant_id,
            installation_id: request.installation_id,
            drained_events,
            drained_schedules,
            remaining_pending,
        })
    }
}
