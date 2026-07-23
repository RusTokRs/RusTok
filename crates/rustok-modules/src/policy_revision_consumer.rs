use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use thiserror::Error;
use uuid::Uuid;

use crate::data::{configure_tenant_scope, now_expression, placeholder, uuid_value};
use crate::{
    ModulePolicyRevisionApplyOutcome, ModulePolicyRevisionGate, ModulePolicyRevisionGateError,
    ModulePolicyRevisionTransition,
};

const MAX_CONSUMER_KEY_BYTES: usize = 128;

/// Durable owner cursor for one tenant-scoped outbox consumer. The cursor is
/// advanced only inside the same transaction that locks its row, so delivery
/// retries and concurrent workers share one predecessor-bound source of truth.
#[derive(Clone)]
pub struct SeaOrmModulePolicyRevisionConsumer {
    db: DatabaseConnection,
}

impl SeaOrmModulePolicyRevisionConsumer {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn apply(
        &self,
        tenant_id: Uuid,
        consumer_key: &str,
        transition: &ModulePolicyRevisionTransition,
    ) -> Result<ModulePolicyRevisionApplyOutcome, ModulePolicyRevisionConsumerError> {
        validate_request(tenant_id, consumer_key)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let outcome = self
            .apply_in_transaction(&transaction, tenant_id, consumer_key, transition)
            .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(outcome)
    }

    /// Locks the durable cursor row on an owner-open transaction and returns its
    /// current revision. Other owner workflows can use the same row as a
    /// tenant-policy serialization point without reading lifecycle-private state.
    pub async fn lock_current_revision_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        consumer_key: &str,
    ) -> Result<Option<String>, ModulePolicyRevisionConsumerError> {
        validate_request(tenant_id, consumer_key)?;
        configure_tenant_scope(transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend = transaction.get_database_backend();
        ensure_cursor_row(transaction, backend, tenant_id, consumer_key).await?;
        load_cursor_revision(transaction, backend, tenant_id, consumer_key).await
    }

    /// Applies a predecessor-bound transition on an owner-open transaction.
    ///
    /// Owner state mutation, outbox append, and cursor advancement can therefore
    /// commit or roll back as one unit. The cursor is still the only durable
    /// consumer projection; this method does not append or acknowledge events.
    pub async fn apply_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        consumer_key: &str,
        transition: &ModulePolicyRevisionTransition,
    ) -> Result<ModulePolicyRevisionApplyOutcome, ModulePolicyRevisionConsumerError> {
        let current_revision = self
            .lock_current_revision_in_transaction(transaction, tenant_id, consumer_key)
            .await?;
        let mut gate = ModulePolicyRevisionGate::new(current_revision)?;
        let outcome = gate.apply(transition)?;
        if outcome == ModulePolicyRevisionApplyOutcome::Applied {
            let backend = transaction.get_database_backend();
            let updated = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_policy_revision_cursors SET current_revision = {}, updated_at = {} \
                         WHERE tenant_id = {} AND consumer_key = {}",
                        placeholder(backend, 1),
                        now_expression(backend),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                    ),
                    vec![
                        transition.next_revision.clone().into(),
                        uuid_value(tenant_id, backend),
                        consumer_key.to_string().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if updated.rows_affected() != 1 {
                return Err(ModulePolicyRevisionConsumerError::CursorLost);
            }
        }
        Ok(outcome)
    }

    pub async fn current_revision(
        &self,
        tenant_id: Uuid,
        consumer_key: &str,
    ) -> Result<Option<String>, ModulePolicyRevisionConsumerError> {
        validate_request(tenant_id, consumer_key)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let revision = load_cursor_revision(
            &transaction,
            transaction.get_database_backend(),
            tenant_id,
            consumer_key,
        )
        .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(revision)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModulePolicyRevisionConsumerError {
    #[error("policy revision consumer tenant must be a non-nil UUID")]
    InvalidTenant,
    #[error("policy revision consumer key is invalid")]
    InvalidConsumerKey,
    #[error(transparent)]
    Revision(#[from] ModulePolicyRevisionGateError),
    #[error("policy revision consumer cursor row was lost")]
    CursorLost,
    #[error("policy revision consumer storage failed: {0}")]
    Storage(String),
}

async fn ensure_cursor_row<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    consumer_key: &str,
) -> Result<(), ModulePolicyRevisionConsumerError> {
    connection
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_policy_revision_cursors \
                 (tenant_id, consumer_key, current_revision, updated_at) \
                 VALUES ({}, {}, {}, {}) \
                 ON CONFLICT (tenant_id, consumer_key) DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
            ),
            vec![
                uuid_value(tenant_id, backend),
                consumer_key.to_string().into(),
                SqlValue::String(None),
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn load_cursor_revision<C: ConnectionTrait>(
    connection: &C,
    backend: DbBackend,
    tenant_id: Uuid,
    consumer_key: &str,
) -> Result<Option<String>, ModulePolicyRevisionConsumerError> {
    let lock = if backend == DbBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT current_revision FROM module_policy_revision_cursors \
                 WHERE tenant_id = {} AND consumer_key = {}{lock}",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![
                uuid_value(tenant_id, backend),
                consumer_key.to_string().into(),
            ],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ModulePolicyRevisionConsumerError::CursorLost)?;
    row.try_get("", "current_revision").map_err(storage_error)
}

fn validate_request(
    tenant_id: Uuid,
    consumer_key: &str,
) -> Result<(), ModulePolicyRevisionConsumerError> {
    if tenant_id.is_nil() {
        return Err(ModulePolicyRevisionConsumerError::InvalidTenant);
    }
    if consumer_key.trim().is_empty()
        || consumer_key != consumer_key.trim()
        || consumer_key.len() > MAX_CONSUMER_KEY_BYTES
        || consumer_key.chars().any(char::is_control)
    {
        return Err(ModulePolicyRevisionConsumerError::InvalidConsumerKey);
    }
    Ok(())
}

fn storage_error(error: impl std::fmt::Display) -> ModulePolicyRevisionConsumerError {
    ModulePolicyRevisionConsumerError::Storage(error.to_string())
}
