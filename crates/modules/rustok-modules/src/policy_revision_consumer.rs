use rustok_core::ModuleRegistry;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, Statement, TransactionTrait,
    Value as SqlValue,
};
use thiserror::Error;
use uuid::Uuid;

use crate::backend::SupportedBackend;
use crate::data::configure_tenant_scope;
use crate::policy::ModuleEffectivePolicyQuery;
use crate::{
    ModuleDefinitionCatalog, ModuleDefinitionSource, ModuleEffectivePolicy,
    ModuleLifecycleDbWriterError, ModulePolicyRevisionApplyOutcome, ModulePolicyRevisionGate,
    ModulePolicyRevisionGateError, ModulePolicyRevisionTransition, TenantModuleOverride,
};

const MAX_CONSUMER_KEY_BYTES: usize = 128;

/// Durable owner cursor for one tenant-scoped outbox consumer. The cursor is
/// advanced only inside the same transaction that locks its row, so delivery
/// retries and concurrent workers share one predecessor-bound source of truth.
#[derive(Clone)]
pub struct SeaOrmModulePolicyRevisionConsumer {
    db: DatabaseConnection,
}

/// One durable policy-revision cursor state. `current_sequence` is the
/// owner-assigned cursor generation: it starts at zero and increments by
/// exactly one for every durable incremental advance or explicit resync. It
/// gives same-tenant/consumer transitions a deterministic order even though
/// revision digests themselves are opaque identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModulePolicyRevisionCursorState {
    pub current_revision: Option<String>,
    pub current_sequence: u64,
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
        Ok(self
            .lock_cursor_state_in_transaction(transaction, tenant_id, consumer_key)
            .await?
            .current_revision)
    }

    /// Locks the durable cursor row and returns both its revision and its
    /// deterministic sequence. Owner transitions must use this locked state,
    /// rather than an unlocked policy projection, as their predecessor.
    pub async fn lock_cursor_state_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        consumer_key: &str,
    ) -> Result<ModulePolicyRevisionCursorState, ModulePolicyRevisionConsumerError> {
        validate_request(tenant_id, consumer_key)?;
        configure_tenant_scope(transaction, tenant_id)
            .await
            .map_err(storage_error)?;
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        ensure_cursor_row(transaction, backend, tenant_id, consumer_key).await?;
        load_cursor_state(transaction, backend, tenant_id, consumer_key).await
    }

    /// Locks the lifecycle policy cursor and resolves the static registry policy
    /// from tenant overrides on the same transaction. This is the commit-time
    /// owner boundary for consumers that must serialize with tenant lifecycle
    /// changes without reading `tenant_modules` themselves.
    pub async fn lock_and_resolve_static_policy_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        consumer_key: &str,
        registry: &ModuleRegistry,
        default_enabled_modules: impl IntoIterator<Item = String>,
    ) -> Result<ModuleEffectivePolicy, ModuleLifecycleDbWriterError> {
        self.lock_current_revision_in_transaction(transaction, tenant_id, consumer_key)
            .await
            .map_err(|error| ModuleLifecycleDbWriterError::PolicyTransition(error.to_string()))?;

        let catalog = ModuleDefinitionCatalog::from_static_registry(registry)
            .map_err(ModuleLifecycleDbWriterError::Definition)?;
        if catalog
            .definitions()
            .any(|definition| matches!(&definition.source, ModuleDefinitionSource::Artifact { .. }))
        {
            return Err(ModuleLifecycleDbWriterError::Configuration(
                "transaction-bound static policy resolver cannot evaluate artifact definitions"
                    .to_string(),
            ));
        }

        ModuleEffectivePolicyQuery::new_with_context(
            &catalog,
            default_enabled_modules,
            load_tenant_overrides(transaction, tenant_id).await?,
            Vec::new(),
            None,
            None,
        )
        .execute()
        .map_err(ModuleLifecycleDbWriterError::Policy)
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
        let current_state = self
            .lock_cursor_state_in_transaction(transaction, tenant_id, consumer_key)
            .await?;
        let mut gate = ModulePolicyRevisionGate::with_sequence(
            current_state.current_revision.clone(),
            current_state.current_sequence,
        )?;
        let outcome = gate.apply(transition)?;
        // Only an exact predecessor advances the cursor. A duplicate is a
        // replay, and a gap leaves the locked state untouched until the
        // caller explicitly resyncs from a full policy snapshot.
        if outcome == ModulePolicyRevisionApplyOutcome::Applied {
            let backend =
                SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
            persist_cursor_state(
                transaction,
                backend,
                tenant_id,
                consumer_key,
                current_state.current_sequence,
                &gate,
            )
            .await?;
        }
        Ok(outcome)
    }

    /// Explicitly resyncs the cursor to a full-snapshot revision, but only if
    /// the locked durable state is still the state the caller based that
    /// snapshot on. This is the sole recovery path after `ResyncRequired`;
    /// incremental application never performs this replacement implicitly.
    pub async fn resync_if_current(
        &self,
        tenant_id: Uuid,
        consumer_key: &str,
        expected_state: &ModulePolicyRevisionCursorState,
        snapshot_revision: &str,
    ) -> Result<ModulePolicyRevisionCursorState, ModulePolicyRevisionConsumerError> {
        validate_request(tenant_id, consumer_key)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let state = self
            .resync_if_current_in_transaction(
                &transaction,
                tenant_id,
                consumer_key,
                expected_state,
                snapshot_revision,
            )
            .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(state)
    }

    pub async fn resync_if_current_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        consumer_key: &str,
        expected_state: &ModulePolicyRevisionCursorState,
        snapshot_revision: &str,
    ) -> Result<ModulePolicyRevisionCursorState, ModulePolicyRevisionConsumerError> {
        // Validate the caller-supplied expected state before comparing it to
        // durable state; an impossible state must not become a resync basis.
        ModulePolicyRevisionGate::with_sequence(
            expected_state.current_revision.clone(),
            expected_state.current_sequence,
        )?;
        let current_state = self
            .lock_cursor_state_in_transaction(transaction, tenant_id, consumer_key)
            .await?;
        if &current_state != expected_state {
            return Err(ModulePolicyRevisionConsumerError::CursorStateConflict {
                expected_sequence: expected_state.current_sequence,
                current_sequence: current_state.current_sequence,
            });
        }
        let mut gate = ModulePolicyRevisionGate::with_sequence(
            current_state.current_revision.clone(),
            current_state.current_sequence,
        )?;
        if !gate.resync_to(snapshot_revision)? {
            return Ok(current_state);
        }
        let backend =
            SupportedBackend::new(transaction.get_database_backend()).map_err(storage_error)?;
        persist_cursor_state(
            transaction,
            backend,
            tenant_id,
            consumer_key,
            current_state.current_sequence,
            &gate,
        )
        .await
    }

    pub async fn current_revision(
        &self,
        tenant_id: Uuid,
        consumer_key: &str,
    ) -> Result<Option<String>, ModulePolicyRevisionConsumerError> {
        Ok(self
            .cursor_state(tenant_id, consumer_key)
            .await?
            .current_revision)
    }

    pub async fn cursor_state(
        &self,
        tenant_id: Uuid,
        consumer_key: &str,
    ) -> Result<ModulePolicyRevisionCursorState, ModulePolicyRevisionConsumerError> {
        validate_request(tenant_id, consumer_key)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let state = self
            .lock_cursor_state_in_transaction(&transaction, tenant_id, consumer_key)
            .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(state)
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
    #[error(
        "policy revision consumer cursor changed: expected sequence {expected_sequence}, found {current_sequence}"
    )]
    CursorStateConflict {
        expected_sequence: u64,
        current_sequence: u64,
    },
    #[error("policy revision consumer storage failed: {0}")]
    Storage(String),
}

async fn ensure_cursor_row<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    tenant_id: Uuid,
    consumer_key: &str,
) -> Result<(), ModulePolicyRevisionConsumerError> {
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "INSERT INTO module_policy_revision_cursors \
                 (tenant_id, consumer_key, current_revision, current_sequence, updated_at) \
                 VALUES ({}, {}, {}, {}, {}) \
                 ON CONFLICT (tenant_id, consumer_key) DO NOTHING",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.now_sql(),
            ),
            vec![
                backend.uuid_value(tenant_id),
                consumer_key.to_string().into(),
                SqlValue::String(None),
                0_i64.into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn load_cursor_state<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    tenant_id: Uuid,
    consumer_key: &str,
) -> Result<ModulePolicyRevisionCursorState, ModulePolicyRevisionConsumerError> {
    let lock = if backend == SupportedBackend::Postgres {
        " FOR UPDATE"
    } else {
        ""
    };
    let row = connection
        .query_one_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "SELECT current_revision, current_sequence \
                 FROM module_policy_revision_cursors \
                 WHERE tenant_id = {} AND consumer_key = {}{lock}",
                backend.placeholder(1),
                backend.placeholder(2),
            ),
            vec![
                backend.uuid_value(tenant_id),
                consumer_key.to_string().into(),
            ],
        ))
        .await
        .map_err(storage_error)?
        .ok_or(ModulePolicyRevisionConsumerError::CursorLost)?;
    let current_revision: Option<String> =
        row.try_get("", "current_revision").map_err(storage_error)?;
    let current_sequence: i64 = row.try_get("", "current_sequence").map_err(storage_error)?;
    let current_sequence = u64::try_from(current_sequence).map_err(|_| {
        ModulePolicyRevisionConsumerError::Storage(format!(
            "policy revision cursor sequence is negative: {current_sequence}"
        ))
    })?;
    // Reuse the gate invariant at the storage boundary so malformed durable
    // state fails closed instead of becoming a transition predecessor.
    ModulePolicyRevisionGate::with_sequence(current_revision.clone(), current_sequence)?;
    Ok(ModulePolicyRevisionCursorState {
        current_revision,
        current_sequence,
    })
}

async fn persist_cursor_state<C: ConnectionTrait>(
    connection: &C,
    backend: SupportedBackend,
    tenant_id: Uuid,
    consumer_key: &str,
    expected_sequence: u64,
    gate: &ModulePolicyRevisionGate,
) -> Result<ModulePolicyRevisionCursorState, ModulePolicyRevisionConsumerError> {
    let next_revision = gate.current_revision().ok_or_else(|| {
        ModulePolicyRevisionConsumerError::Storage(
            "advanced policy revision gate has no current revision".to_string(),
        )
    })?;
    let next_sequence = gate.current_sequence().ok_or_else(|| {
        ModulePolicyRevisionConsumerError::Storage(
            "advanced policy revision gate has no current sequence".to_string(),
        )
    })?;
    let expected_sequence_i64 = i64::try_from(expected_sequence).map_err(|_| {
        ModulePolicyRevisionConsumerError::Storage(format!(
            "policy revision cursor sequence exceeds durable range: {expected_sequence}"
        ))
    })?;
    let next_sequence_i64 = i64::try_from(next_sequence).map_err(|_| {
        ModulePolicyRevisionConsumerError::Storage(format!(
            "policy revision cursor sequence exceeds durable range: {next_sequence}"
        ))
    })?;
    let updated = connection
        .execute_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            format!(
                "UPDATE module_policy_revision_cursors \
                 SET current_revision = {}, current_sequence = {}, updated_at = {} \
                 WHERE tenant_id = {} AND consumer_key = {} AND current_sequence = {}",
                backend.placeholder(1),
                backend.placeholder(2),
                backend.now_sql(),
                backend.placeholder(3),
                backend.placeholder(4),
                backend.placeholder(5),
            ),
            vec![
                next_revision.to_string().into(),
                next_sequence_i64.into(),
                backend.uuid_value(tenant_id),
                consumer_key.to_string().into(),
                expected_sequence_i64.into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    if updated.rows_affected() != 1 {
        return Err(ModulePolicyRevisionConsumerError::CursorLost);
    }
    Ok(ModulePolicyRevisionCursorState {
        current_revision: Some(next_revision.to_string()),
        current_sequence: next_sequence,
    })
}

async fn load_tenant_overrides<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
) -> Result<Vec<TenantModuleOverride>, ModuleLifecycleDbWriterError> {
    let backend = SupportedBackend::new(connection.get_database_backend())
        .map_err(|error| ModuleLifecycleDbWriterError::Database(error.to_string()))?;
    let sql = match backend {
        SupportedBackend::Postgres => {
            "SELECT module_slug, enabled FROM tenant_modules \
             WHERE tenant_id = $1 AND enabled IS NOT NULL"
        }
        SupportedBackend::Sqlite => {
            "SELECT module_slug, enabled FROM tenant_modules \
             WHERE tenant_id = ?1 AND enabled IS NOT NULL"
        }
    };
    connection
        .query_all_raw(Statement::from_sql_and_values(
            backend.db_backend(),
            sql,
            vec![backend.uuid_value(tenant_id)],
        ))
        .await
        .map_err(|error| ModuleLifecycleDbWriterError::Database(error.to_string()))?
        .into_iter()
        .map(|row| {
            Ok(TenantModuleOverride {
                module_slug: row
                    .try_get("", "module_slug")
                    .map_err(|error| ModuleLifecycleDbWriterError::Database(error.to_string()))?,
                enabled: row
                    .try_get("", "enabled")
                    .map_err(|error| ModuleLifecycleDbWriterError::Database(error.to_string()))?,
            })
        })
        .collect()
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
