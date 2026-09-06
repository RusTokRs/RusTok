//! Durable owner service for module transition convergence.

use rustok_events::DomainEvent;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ControlPlaneInfrastructure, ModuleTransitionCheckpoint, ModuleTransitionCoordinator,
    ModuleTransitionFinalizeCommand, RetentionHoldStore, SecurityEpochRegistry,
    TransitionCheckpointStore, TransitionCoordinatorError, TransitionStoreError,
    data::{now_expression, placeholder, uuid_from_row, uuid_value},
};

const OPERATION_KIND: &str = "finalize";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModuleTransitionFinalizeReceipt {
    pub checkpoint: ModuleTransitionCheckpoint,
    pub released_holds: u64,
    pub created: bool,
}

#[derive(Debug, Error)]
pub enum ModuleTransitionServiceError {
    #[error("transition command is invalid: {0}")]
    InvalidCommand(String),
    #[error("module transition authorization denied")]
    AuthorizationDenied,
    #[error("transition checkpoint `{0}` was not found")]
    NotFound(Uuid),
    #[error("transition revision conflict: expected `{expected}`, current `{current}")]
    RevisionConflict { expected: u64, current: u64 },
    #[error("transition idempotency key was used for a different command")]
    IdempotencyConflict,
    #[error("transition operation is still in progress")]
    OperationInProgress,
    #[error(transparent)]
    Coordinator(#[from] TransitionCoordinatorError),
    #[error("transition store failed: {0}")]
    Store(String),
    #[error("transition outbox append failed: {0}")]
    Outbox(String),
}

#[derive(Clone)]
pub struct SeaOrmModuleTransitionService {
    db: DatabaseConnection,
    infrastructure: ControlPlaneInfrastructure,
}

impl SeaOrmModuleTransitionService {
    pub(crate) fn with_infrastructure(
        db: DatabaseConnection,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self { db, infrastructure }
    }

    pub async fn checkpoint(
        &self,
        operation_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<Option<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
        let checkpoint = TransitionCheckpointStore::load_checkpoint(&self.db, operation_id)
            .await
            .map_err(store_error)?;
        Ok(checkpoint.filter(|checkpoint| checkpoint.tenant_id == tenant_id))
    }

    pub async fn active_checkpoints(
        &self,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
        let checkpoints = TransitionCheckpointStore::list_active_checkpoints(&self.db)
            .await
            .map_err(store_error)?;
        Ok(checkpoints
            .into_iter()
            .filter(|checkpoint| checkpoint.tenant_id == tenant_id)
            .collect())
    }

    pub async fn retention_holds(
        &self,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<crate::RetentionHoldRecord>, ModuleTransitionServiceError> {
        let operation_ids = self
            .active_checkpoints(tenant_id)
            .await?
            .into_iter()
            .map(|checkpoint| checkpoint.operation_id)
            .collect::<std::collections::HashSet<_>>();
        let holds = RetentionHoldStore::list_active_holds(&self.db)
            .await
            .map_err(store_error)?;
        Ok(holds
            .into_iter()
            .filter(|hold| {
                matches!(
                    hold.kind,
                    crate::RetentionHoldKind::ActiveRolloutWindow { operation_id, .. }
                        if operation_ids.contains(&operation_id)
                )
            })
            .collect())
    }

    pub async fn finalize(
        &self,
        command: ModuleTransitionFinalizeCommand,
    ) -> Result<ModuleTransitionFinalizeReceipt, ModuleTransitionServiceError> {
        validate_command(&command)?;
        if !command.actor_can_manage_modules {
            return Err(ModuleTransitionServiceError::AuthorizationDenied);
        }
        let request_digest = request_digest(&command)?;
        if let Some(receipt) = load_operation(&self.db, &command, &request_digest).await? {
            return Ok(receipt);
        }

        let transaction = self.db.begin().await.map_err(database_error)?;
        if let Some(receipt) = load_operation(&transaction, &command, &request_digest).await? {
            return Ok(receipt);
        }
        let checkpoint = TransitionCheckpointStore::load_checkpoint(
            &transaction,
            command.operation_id,
        )
        .await
        .map_err(store_error)?
        .ok_or(ModuleTransitionServiceError::NotFound(command.operation_id))?;
        if checkpoint.tenant_id != command.context.tenant_id {
            return Err(ModuleTransitionServiceError::AuthorizationDenied);
        }
        if checkpoint.revision != command.expected_revision {
            return Err(ModuleTransitionServiceError::RevisionConflict {
                expected: command.expected_revision,
                current: checkpoint.revision,
            });
        }
        if let Some(receipt) =
            reserve_operation(&transaction, &command, &request_digest).await?
        {
            return Ok(receipt);
        }

        let security_registry = SecurityEpochRegistry::new();
        let mut coordinator = ModuleTransitionCoordinator::new(checkpoint);
        coordinator.finalize_convergence(&security_registry)?;
        TransitionCheckpointStore::save_checkpoint(&transaction, coordinator.checkpoint())
            .await
            .map_err(store_error)?;
        let released_holds = RetentionHoldStore::release_holds_for_operation(
            &transaction,
            command.operation_id,
        )
        .await
        .map_err(store_error)?;
        let receipt = ModuleTransitionFinalizeReceipt {
            checkpoint: coordinator.checkpoint().clone(),
            released_holds,
            created: true,
        };
        complete_operation(&transaction, command.context.idempotency_key, &receipt).await?;
        self.infrastructure
            .write_event(
                &transaction,
                self.infrastructure.event_envelope_for_command(
                    &command.context,
                    DomainEvent::ModuleTransitionFinalized {
                        operation_id: command.operation_id,
                        module_slug: receipt.checkpoint.module_slug.clone(),
                        revision: receipt.checkpoint.revision,
                        released_holds,
                    },
                ),
            )
            .await
            .map_err(|error| ModuleTransitionServiceError::Outbox(error.to_string()))?;
        transaction.commit().await.map_err(database_error)?;
        Ok(receipt)
    }

    pub async fn evaluate_watchdog(
        &self,
        security_registry: &SecurityEpochRegistry,
    ) -> Result<Vec<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
        let active = TransitionCheckpointStore::list_active_checkpoints(&self.db)
            .await
            .map_err(store_error)?;
        let mut updated = Vec::new();
        for observed in active {
            let stale_epoch = security_registry
                .validate_epoch(observed.security_epoch)
                .is_err();
            let timed_out = matches!(
                observed.state,
                crate::ModuleTransitionState::Observing { timeout_at }
                    if self.infrastructure.now() >= timeout_at
            );
            if !stale_epoch && !timed_out {
                continue;
            }

            let transaction = self.db.begin().await.map_err(database_error)?;
            let current = TransitionCheckpointStore::load_checkpoint(
                &transaction,
                observed.operation_id,
            )
            .await
            .map_err(store_error)?
            .ok_or(ModuleTransitionServiceError::NotFound(observed.operation_id))?;
            if current.revision != observed.revision || current.state.is_terminal() {
                continue;
            }
            let mut coordinator = ModuleTransitionCoordinator::new(current);
            if stale_epoch {
                let reason = format!(
                    "Security epoch preemption: epoch {} is stale; current epoch is {}. Automatic rollback requires a fresh owner policy grant and was not attempted.",
                    coordinator.checkpoint().security_epoch.value(),
                    security_registry.current_epoch().value(),
                );
                coordinator.fail_closed(reason.clone())?;
                TransitionCheckpointStore::save_checkpoint(
                    &transaction,
                    coordinator.checkpoint(),
                )
                .await
                .map_err(store_error)?;
                self.infrastructure
                    .write_event(
                        &transaction,
                        self.infrastructure.event_envelope(
                            coordinator.checkpoint().tenant_id,
                            None,
                            DomainEvent::ModuleTransitionFailedClosed {
                                operation_id: coordinator.checkpoint().operation_id,
                                module_slug: coordinator.checkpoint().module_slug.clone(),
                                revision: coordinator.checkpoint().revision,
                                failure_reason: reason,
                            },
                        ),
                    )
                    .await
                    .map_err(|error| ModuleTransitionServiceError::Outbox(error.to_string()))?;
            } else {
                coordinator.finalize_convergence(security_registry)?;
                TransitionCheckpointStore::save_checkpoint(
                    &transaction,
                    coordinator.checkpoint(),
                )
                .await
                .map_err(store_error)?;
                let released_holds = RetentionHoldStore::release_holds_for_operation(
                    &transaction,
                    coordinator.checkpoint().operation_id,
                )
                .await
                .map_err(store_error)?;
                self.infrastructure
                    .write_event(
                        &transaction,
                        self.infrastructure.event_envelope(
                            coordinator.checkpoint().tenant_id,
                            None,
                            DomainEvent::ModuleTransitionFinalized {
                                operation_id: coordinator.checkpoint().operation_id,
                                module_slug: coordinator.checkpoint().module_slug.clone(),
                                revision: coordinator.checkpoint().revision,
                                released_holds,
                            },
                        ),
                    )
                    .await
                    .map_err(|error| ModuleTransitionServiceError::Outbox(error.to_string()))?;
            }
            transaction.commit().await.map_err(database_error)?;
            updated.push(coordinator.checkpoint().clone());
        }
        Ok(updated)
    }
}

pub async fn evaluate_transition_watchdog(
    db: &DatabaseConnection,
    security_registry: &SecurityEpochRegistry,
) -> Result<Vec<ModuleTransitionCheckpoint>, ModuleTransitionServiceError> {
    SeaOrmModuleTransitionService::with_infrastructure(
        db.clone(),
        ControlPlaneInfrastructure::for_database(db.clone()),
    )
    .evaluate_watchdog(security_registry)
    .await
}

fn validate_command(
    command: &ModuleTransitionFinalizeCommand,
) -> Result<(), ModuleTransitionServiceError> {
    if command.operation_id.is_nil() || command.expected_revision == 0 {
        return Err(ModuleTransitionServiceError::InvalidCommand(
            "operation identity and expected revision must be positive".to_string(),
        ));
    }
    command
        .context
        .validate()
        .map_err(|error| ModuleTransitionServiceError::InvalidCommand(error.to_string()))
}

fn request_digest(
    command: &ModuleTransitionFinalizeCommand,
) -> Result<String, ModuleTransitionServiceError> {
    let bytes = serde_json::to_vec(command)
        .map_err(|error| ModuleTransitionServiceError::Store(error.to_string()))?;
    Ok(format!("sha256:{}", hex::encode(Sha256::digest(bytes))))
}

async fn reserve_operation(
    transaction: &DatabaseTransaction,
    command: &ModuleTransitionFinalizeCommand,
    request_digest: &str,
) -> Result<Option<ModuleTransitionFinalizeReceipt>, ModuleTransitionServiceError> {
    let backend = transaction.get_database_backend();
    let inserted = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_transition_operations \
                 (idempotency_key, operation_kind, request_digest, actor_id, tenant_id, trace_id, \
                  correlation_id, operation_id, created_at) \
                 VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}) \
                 ON CONFLICT (idempotency_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                placeholder(backend, 7),
                placeholder(backend, 8),
                now_expression(backend),
            ),
            vec![
                uuid_value(command.context.idempotency_key, backend),
                OPERATION_KIND.into(),
                request_digest.to_owned().into(),
                uuid_value(command.context.actor_id, backend),
                command
                    .context
                    .tenant_id
                    .map(|tenant_id| uuid_value(tenant_id, backend))
                    .unwrap_or(sea_orm::Value::Uuid(None)),
                command.context.trace_id.clone().into(),
                uuid_value(command.context.correlation_id, backend),
                uuid_value(command.operation_id, backend),
            ],
        ))
        .await
        .map_err(database_error)?;
    if inserted.rows_affected() == 1 {
        return Ok(None);
    }
    load_operation(transaction, command, request_digest).await
}

async fn load_operation<C: ConnectionTrait>(
    connection: &C,
    command: &ModuleTransitionFinalizeCommand,
    request_digest: &str,
) -> Result<Option<ModuleTransitionFinalizeReceipt>, ModuleTransitionServiceError> {
    let backend = connection.get_database_backend();
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT operation_kind, request_digest, actor_id, tenant_id, trace_id, \
                 correlation_id, operation_id, receipt_json \
                 FROM module_transition_operations WHERE idempotency_key = {}",
                placeholder(backend, 1),
            ),
            vec![uuid_value(command.context.idempotency_key, backend)],
        ))
        .await
        .map_err(database_error)?;
    let Some(row) = row else {
        return Ok(None);
    };
    validate_operation_row(&row, backend, command, request_digest)?;
    let receipt_json: Option<String> = row.try_get("", "receipt_json").map_err(database_error)?;
    let Some(receipt_json) = receipt_json else {
        return Err(ModuleTransitionServiceError::OperationInProgress);
    };
    let mut receipt: ModuleTransitionFinalizeReceipt = serde_json::from_str(&receipt_json)
        .map_err(|error| ModuleTransitionServiceError::Store(error.to_string()))?;
    receipt.created = false;
    Ok(Some(receipt))
}

fn validate_operation_row(
    row: &QueryResult,
    backend: DbBackend,
    command: &ModuleTransitionFinalizeCommand,
    request_digest: &str,
) -> Result<(), ModuleTransitionServiceError> {
    let stored_tenant = match row.try_get::<Option<Uuid>>("", "tenant_id") {
        Ok(value) => value,
        Err(_) if backend != DbBackend::Postgres => row
            .try_get::<Option<String>>("", "tenant_id")
            .map_err(database_error)?
            .map(|value| Uuid::parse_str(&value).map_err(database_error))
            .transpose()?,
        Err(error) => return Err(database_error(error)),
    };
    let matches = row
        .try_get::<String>("", "operation_kind")
        .map_err(database_error)?
        == OPERATION_KIND
        && row
            .try_get::<String>("", "request_digest")
            .map_err(database_error)?
            == request_digest
        && uuid_from_row(row, "actor_id", backend).map_err(database_error)?
            == command.context.actor_id
        && stored_tenant == command.context.tenant_id
        && row.try_get::<String>("", "trace_id").map_err(database_error)?
            == command.context.trace_id
        && uuid_from_row(row, "correlation_id", backend).map_err(database_error)?
            == command.context.correlation_id
        && uuid_from_row(row, "operation_id", backend).map_err(database_error)?
            == command.operation_id;
    if matches {
        Ok(())
    } else {
        Err(ModuleTransitionServiceError::IdempotencyConflict)
    }
}

async fn complete_operation(
    transaction: &DatabaseTransaction,
    idempotency_key: Uuid,
    receipt: &ModuleTransitionFinalizeReceipt,
) -> Result<(), ModuleTransitionServiceError> {
    let backend = transaction.get_database_backend();
    let receipt_json = serde_json::to_string(receipt)
        .map_err(|error| ModuleTransitionServiceError::Store(error.to_string()))?;
    let result = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "UPDATE module_transition_operations SET resulting_revision = {}, \
                 receipt_json = {}, completed_at = {} \
                 WHERE idempotency_key = {} AND completed_at IS NULL",
                placeholder(backend, 1),
                placeholder(backend, 2),
                now_expression(backend),
                placeholder(backend, 3),
            ),
            vec![
                i64::try_from(receipt.checkpoint.revision)
                    .map_err(|_| ModuleTransitionServiceError::Store(
                        "transition revision exceeds database range".to_string(),
                    ))?
                    .into(),
                receipt_json.into(),
                uuid_value(idempotency_key, backend),
            ],
        ))
        .await
        .map_err(database_error)?;
    if result.rows_affected() != 1 {
        return Err(ModuleTransitionServiceError::OperationInProgress);
    }
    Ok(())
}

fn store_error(error: TransitionStoreError) -> ModuleTransitionServiceError {
    match error {
        TransitionStoreError::CheckpointNotFound(operation_id) => {
            ModuleTransitionServiceError::NotFound(operation_id)
        }
        TransitionStoreError::RevisionConflict {
            expected, current, ..
        } => ModuleTransitionServiceError::RevisionConflict { expected, current },
        error => ModuleTransitionServiceError::Store(error.to_string()),
    }
}

fn database_error(error: impl std::fmt::Display) -> ModuleTransitionServiceError {
    ModuleTransitionServiceError::Store(error.to_string())
}
