use std::{fmt, sync::Arc};

use async_trait::async_trait;
use sea_orm::{
    AccessMode, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend,
    IsolationLevel, QueryResult, Statement, TransactionTrait,
};
use uuid::Uuid;

use crate::{
    IndexDriftFindingAuthorizedLifecycleCommand, IndexDriftFindingLifecycleCommand,
    IndexDriftFindingLifecycleFailure, IndexDriftFindingLifecycleNotAppliedReason,
    IndexDriftFindingLifecycleReceipt, IndexDriftFindingLifecycleStore,
    IndexDriftFindingLifecycleStoreOutcome, IndexDriftFindingState,
};

const STORAGE_UNAVAILABLE: &str = "index_drift_finding_lifecycle_storage_unavailable";
const STORED_CONTRACT_INVALID: &str = "index_drift_finding_lifecycle_stored_contract_invalid";
const COMMAND_ID_CONFLICT: &str = "index_drift_finding_lifecycle_command_id_conflict";
const UNSUPPORTED_BACKEND: &str = "index_drift_finding_lifecycle_unsupported_backend";

#[derive(Clone)]
pub struct PostgresIndexDriftFindingLifecycleStore {
    db: DatabaseConnection,
}

impl PostgresIndexDriftFindingLifecycleStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    async fn apply_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        command: &IndexDriftFindingLifecycleCommand,
    ) -> Result<IndexDriftFindingLifecycleStoreOutcome, IndexDriftFindingLifecycleFailure> {
        lock_command_id(transaction, command).await?;
        if let Some(event) = load_existing_event(transaction, command).await? {
            if !event.matches(command) {
                return Err(permanent_failure(COMMAND_ID_CONFLICT));
            }
            return Ok(IndexDriftFindingLifecycleStoreOutcome::AlreadyApplied(
                receipt(command),
            ));
        }

        let Some(current_state) = lock_finding_state(transaction, command).await? else {
            return Ok(IndexDriftFindingLifecycleStoreOutcome::NotApplied(
                IndexDriftFindingLifecycleNotAppliedReason::FindingNotFound,
            ));
        };
        if current_state != command.expected_state() {
            return Ok(IndexDriftFindingLifecycleStoreOutcome::NotApplied(
                IndexDriftFindingLifecycleNotAppliedReason::StateChanged {
                    current: current_state,
                },
            ));
        }

        let updated = transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "UPDATE index_consistency_findings SET state = $3, closed_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND finding_id = $2 AND state = $4 AND closed_at IS NULL",
                vec![
                    command.tenant_id().into(),
                    command.finding_id().into(),
                    command.target_state().as_str().into(),
                    command.expected_state().as_str().into(),
                ],
            ))
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        if updated.rows_affected() != 1 {
            return Err(retryable_failure(STORAGE_UNAVAILABLE));
        }

        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "INSERT INTO index_consistency_finding_lifecycle_events (tenant_id, command_id, finding_id, action, from_state, to_state, actor_kind, actor_subject, reason) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)",
                vec![
                    command.tenant_id().into(),
                    command.command_id().into(),
                    command.finding_id().into(),
                    command.action().as_str().into(),
                    command.expected_state().as_str().into(),
                    command.target_state().as_str().into(),
                    command.actor().kind().to_owned().into(),
                    command.actor().subject().to_owned().into(),
                    command.reason().to_owned().into(),
                ],
            ))
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        if inserted.rows_affected() != 1 {
            return Err(retryable_failure(STORAGE_UNAVAILABLE));
        }

        Ok(IndexDriftFindingLifecycleStoreOutcome::Applied(receipt(
            command,
        )))
    }
}

#[async_trait]
impl IndexDriftFindingLifecycleStore for PostgresIndexDriftFindingLifecycleStore {
    async fn apply_authorized_lifecycle_command(
        &self,
        authorized: &IndexDriftFindingAuthorizedLifecycleCommand,
    ) -> Result<IndexDriftFindingLifecycleStoreOutcome, IndexDriftFindingLifecycleFailure> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(permanent_failure(UNSUPPORTED_BACKEND));
        }
        let command = authorized.command();
        let transaction = self
            .db
            .begin_with_config(
                Some(IsolationLevel::Serializable),
                Some(AccessMode::ReadWrite),
            )
            .await
            .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
        let result = self.apply_in_transaction(&transaction, command).await;
        match result {
            Ok(IndexDriftFindingLifecycleStoreOutcome::Applied(outcome)) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
                Ok(IndexDriftFindingLifecycleStoreOutcome::Applied(outcome))
            }
            Ok(IndexDriftFindingLifecycleStoreOutcome::AlreadyApplied(outcome)) => {
                transaction
                    .commit()
                    .await
                    .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
                Ok(IndexDriftFindingLifecycleStoreOutcome::AlreadyApplied(
                    outcome,
                ))
            }
            Ok(IndexDriftFindingLifecycleStoreOutcome::NotApplied(reason)) => {
                transaction
                    .rollback()
                    .await
                    .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
                Ok(IndexDriftFindingLifecycleStoreOutcome::NotApplied(reason))
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

impl fmt::Debug for PostgresIndexDriftFindingLifecycleStore {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("PostgresIndexDriftFindingLifecycleStore")
            .finish_non_exhaustive()
    }
}

pub fn materialize_postgres_index_drift_finding_lifecycle_store(
    db: DatabaseConnection,
) -> Result<Arc<dyn IndexDriftFindingLifecycleStore>, IndexDriftFindingLifecycleFailure> {
    if db.get_database_backend() != DbBackend::Postgres {
        return Err(permanent_failure(UNSUPPORTED_BACKEND));
    }
    Ok(Arc::new(PostgresIndexDriftFindingLifecycleStore::new(db)))
}

#[derive(Clone, PartialEq, Eq)]
struct StoredLifecycleEvent {
    finding_id: Uuid,
    action: String,
    from_state: String,
    to_state: String,
    actor_kind: String,
    actor_subject: String,
    reason: String,
}

impl StoredLifecycleEvent {
    fn matches(&self, command: &IndexDriftFindingLifecycleCommand) -> bool {
        self.finding_id == command.finding_id()
            && self.action == command.action().as_str()
            && self.from_state == command.expected_state().as_str()
            && self.to_state == command.target_state().as_str()
            && self.actor_kind == command.actor().kind()
            && self.actor_subject == command.actor().subject()
            && self.reason == command.reason()
    }
}

async fn lock_command_id(
    transaction: &DatabaseTransaction,
    command: &IndexDriftFindingLifecycleCommand,
) -> Result<(), IndexDriftFindingLifecycleFailure> {
    let key = format!(
        "index-drift-finding-lifecycle\u{1f}{}\u{1f}{}",
        command.tenant_id(),
        command.command_id(),
    );
    transaction
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![key.into()],
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?;
    Ok(())
}

async fn load_existing_event(
    transaction: &DatabaseTransaction,
    command: &IndexDriftFindingLifecycleCommand,
) -> Result<Option<StoredLifecycleEvent>, IndexDriftFindingLifecycleFailure> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT finding_id, action, from_state, to_state, actor_kind, actor_subject, reason FROM index_consistency_finding_lifecycle_events WHERE tenant_id = $1 AND command_id = $2 LIMIT 1",
            vec![command.tenant_id().into(), command.command_id().into()],
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?
        .map(decode_event)
        .transpose()
}

fn decode_event(
    row: QueryResult,
) -> Result<StoredLifecycleEvent, IndexDriftFindingLifecycleFailure> {
    let finding_id = row
        .try_get::<Uuid>("", "finding_id")
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    if finding_id.is_nil() {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    let event = StoredLifecycleEvent {
        finding_id,
        action: required_text(&row, "action")?,
        from_state: required_text(&row, "from_state")?,
        to_state: required_text(&row, "to_state")?,
        actor_kind: required_text(&row, "actor_kind")?,
        actor_subject: required_text(&row, "actor_subject")?,
        reason: required_text(&row, "reason")?,
    };
    if !matches!(event.action.as_str(), "resolve" | "ignore")
        || event.from_state != "open"
        || !matches!(event.to_state.as_str(), "resolved" | "ignored")
    {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    Ok(event)
}

async fn lock_finding_state(
    transaction: &DatabaseTransaction,
    command: &IndexDriftFindingLifecycleCommand,
) -> Result<Option<IndexDriftFindingState>, IndexDriftFindingLifecycleFailure> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT state FROM index_consistency_findings WHERE tenant_id = $1 AND finding_id = $2 FOR UPDATE",
            vec![command.tenant_id().into(), command.finding_id().into()],
        ))
        .await
        .map_err(|_| retryable_failure(STORAGE_UNAVAILABLE))?
        .map(|row| {
            let state = required_text(&row, "state")?;
            parse_state(&state)
        })
        .transpose()
}

fn parse_state(value: &str) -> Result<IndexDriftFindingState, IndexDriftFindingLifecycleFailure> {
    match value {
        "open" => Ok(IndexDriftFindingState::Open),
        "resolved" => Ok(IndexDriftFindingState::Resolved),
        "ignored" => Ok(IndexDriftFindingState::Ignored),
        _ => Err(permanent_failure(STORED_CONTRACT_INVALID)),
    }
}

fn required_text(
    row: &QueryResult,
    column: &str,
) -> Result<String, IndexDriftFindingLifecycleFailure> {
    let value = row
        .try_get::<String>("", column)
        .map_err(|_| permanent_failure(STORED_CONTRACT_INVALID))?;
    if value.is_empty() {
        return Err(permanent_failure(STORED_CONTRACT_INVALID));
    }
    Ok(value)
}

fn receipt(command: &IndexDriftFindingLifecycleCommand) -> IndexDriftFindingLifecycleReceipt {
    IndexDriftFindingLifecycleReceipt::new(
        command.command_id(),
        command.finding_id(),
        command.target_state(),
    )
}

fn retryable_failure(code: &str) -> IndexDriftFindingLifecycleFailure {
    IndexDriftFindingLifecycleFailure::retryable(code)
        .expect("static lifecycle retryable code is valid")
}

fn permanent_failure(code: &str) -> IndexDriftFindingLifecycleFailure {
    IndexDriftFindingLifecycleFailure::permanent(code)
        .expect("static lifecycle permanent code is valid")
}
