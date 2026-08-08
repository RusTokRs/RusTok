use async_trait::async_trait;
use rust_decimal::Decimal;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait,
    Value as SqlValue,
};
use serde_json::Value as JsonValue;

use crate::{
    IndexMutation, IndexReplayCheckpoint, IndexReplayCheckpointKey, IndexReplayCheckpointStore,
    IndexReplayFailure, IndexReplayMutationOutcome, IndexReplayMutationSink, SchemaRegistry,
};

use super::{
    mutation_store::{
        MutationApplyOutcome, MutationDelivery, MutationStorageError, PostgresMutationStore,
    },
    source_replay_job::{
        assert_active_replay_job_lease, IndexReplayJobError, IndexReplayJobLease,
    },
    source_replay_timeout::{
        bounded_replay_checkpoint_commit, bounded_replay_checkpoint_read, bounded_replay_mutation,
    },
};

#[async_trait]
impl IndexReplayMutationSink for PostgresMutationStore {
    async fn apply_replay_mutation(
        &self,
        registry: &SchemaRegistry,
        source_name: &str,
        mutation: &IndexMutation,
    ) -> Result<IndexReplayMutationOutcome, IndexReplayFailure> {
        let delivery = MutationDelivery::from_event(source_name, mutation.clone())
            .map_err(classify_mutation_failure)?;
        bounded_replay_mutation(async {
            self.apply(registry, &delivery)
                .await
                .map(|outcome| match outcome {
                    MutationApplyOutcome::Applied { .. } => IndexReplayMutationOutcome::Applied,
                    MutationApplyOutcome::Duplicate { .. } => IndexReplayMutationOutcome::Duplicate,
                    MutationApplyOutcome::StaleIgnored { .. } => {
                        IndexReplayMutationOutcome::StaleIgnored
                    }
                })
                .map_err(classify_mutation_failure)
        })
        .await
    }
}

#[derive(Clone)]
pub struct PostgresIndexReplayCheckpointStore {
    db: DatabaseConnection,
    lease: IndexReplayJobLease,
}

impl PostgresIndexReplayCheckpointStore {
    pub fn new(db: DatabaseConnection, lease: IndexReplayJobLease) -> Self {
        Self { db, lease }
    }

    pub fn lease(&self) -> &IndexReplayJobLease {
        &self.lease
    }
}

#[async_trait]
impl IndexReplayCheckpointStore for PostgresIndexReplayCheckpointStore {
    async fn load_replay_checkpoint(
        &self,
        key: &IndexReplayCheckpointKey,
    ) -> Result<Option<IndexReplayCheckpoint>, IndexReplayFailure> {
        validate_checkpoint_identity(&self.lease, key)?;
        bounded_replay_checkpoint_read(async {
            let transaction = self
                .db
                .begin()
                .await
                .map_err(|error| checkpoint_storage_failure("checkpoint_read_failed", error))?;
            let result = async {
                let backend = transaction.get_database_backend();
                ensure_supported_backend(backend)?;
                assert_active_replay_job_lease(&transaction, &self.lease, backend)
                    .await
                    .map_err(classify_job_lease_failure)?;
                let row = transaction
                    .query_one(Statement::from_sql_and_values(
                        backend,
                        select_checkpoint_sql(backend),
                        checkpoint_key_values(key, backend),
                    ))
                    .await
                    .map_err(|error| checkpoint_storage_failure("checkpoint_read_failed", error))?;
                let Some(row) = row else {
                    return Ok(None);
                };

                let cursor_json: JsonValue = row.try_get("", "cursor").map_err(|error| {
                    checkpoint_contract_failure("checkpoint_cursor_invalid", error)
                })?;
                let cursor = serde_json::from_value(cursor_json).map_err(|error| {
                    checkpoint_contract_failure("checkpoint_cursor_invalid", error)
                })?;
                let source_version_text: Option<String> = row
                    .try_get("", "source_version_text")
                    .map_err(|error| {
                        checkpoint_contract_failure("checkpoint_source_version_invalid", error)
                    })?;
                let source_version = source_version_text
                    .map(|value| {
                        value.parse::<u64>().map_err(|error| {
                            checkpoint_contract_failure("checkpoint_source_version_invalid", error)
                        })
                    })
                    .transpose()?;
                let last_delivery_id: Option<String> = row.try_get("", "last_delivery_id").map_err(
                    |error| checkpoint_contract_failure("checkpoint_delivery_invalid", error),
                )?;

                IndexReplayCheckpoint::new(key.clone(), cursor, source_version, last_delivery_id)
                    .map(Some)
                    .map_err(|error| {
                        checkpoint_contract_failure("checkpoint_contract_invalid", error)
                    })
            }
            .await;

            match result {
                Ok(checkpoint) => {
                    transaction
                        .commit()
                        .await
                        .map_err(|error| checkpoint_storage_failure("checkpoint_read_failed", error))?;
                    Ok(checkpoint)
                }
                Err(error) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(|rollback| {
                            checkpoint_storage_failure("checkpoint_read_rollback_failed", rollback)
                        })?;
                    Err(error)
                }
            }
        })
        .await
    }

    async fn commit_replay_checkpoint(
        &self,
        checkpoint: &IndexReplayCheckpoint,
    ) -> Result<(), IndexReplayFailure> {
        validate_checkpoint_identity(&self.lease, checkpoint.key())?;
        bounded_replay_checkpoint_commit(async {
            let transaction = self
                .db
                .begin()
                .await
                .map_err(|error| checkpoint_storage_failure("checkpoint_commit_failed", error))?;
            let result = async {
                let backend = transaction.get_database_backend();
                ensure_supported_backend(backend)?;
                assert_active_replay_job_lease(&transaction, &self.lease, backend)
                    .await
                    .map_err(classify_job_lease_failure)?;
                let cursor = serde_json::to_value(checkpoint.cursor()).map_err(|error| {
                    checkpoint_contract_failure("checkpoint_cursor_invalid", error)
                })?;
                let mut values = checkpoint_key_values(checkpoint.key(), backend);
                values.push(SqlValue::Json(Some(Box::new(cursor))));
                values.push(optional_source_version_value(
                    checkpoint.source_version(),
                    backend,
                )?);
                values.push(
                    checkpoint
                        .last_delivery_id()
                        .map(str::to_owned)
                        .into(),
                );

                transaction
                    .execute(Statement::from_sql_and_values(
                        backend,
                        upsert_checkpoint_sql(backend),
                        values,
                    ))
                    .await
                    .map_err(|error| checkpoint_storage_failure("checkpoint_commit_failed", error))?;
                Ok(())
            }
            .await;

            match result {
                Ok(()) => transaction
                    .commit()
                    .await
                    .map_err(|error| checkpoint_storage_failure("checkpoint_commit_failed", error)),
                Err(error) => {
                    transaction
                        .rollback()
                        .await
                        .map_err(|rollback| {
                            checkpoint_storage_failure("checkpoint_commit_rollback_failed", rollback)
                        })?;
                    Err(error)
                }
            }
        })
        .await
    }
}

fn validate_checkpoint_identity(
    lease: &IndexReplayJobLease,
    key: &IndexReplayCheckpointKey,
) -> Result<(), IndexReplayFailure> {
    if key.tenant_id() != lease.tenant_id()
        || key.source_name() != lease.source_name()
        || key.schema() != lease.schema()
        || key.locale() != lease.locale()
    {
        return Err(IndexReplayFailure::permanent_static(
            "checkpoint_lease_identity_mismatch",
        ));
    }
    Ok(())
}

fn classify_job_lease_failure(error: IndexReplayJobError) -> IndexReplayFailure {
    let (retryable, code) = match &error {
        IndexReplayJobError::Storage(_) => (true, "checkpoint_lease_storage_retryable"),
        IndexReplayJobError::LeaseLost => (false, "checkpoint_lease_lost"),
        _ => (false, "checkpoint_lease_contract_invalid"),
    };
    tracing::error!(
        error = ?error,
        replay_failure_code = code,
        replay_failure_retryable = retryable,
        "Index replay checkpoint lease validation failed"
    );
    if retryable {
        IndexReplayFailure::retryable_static(code)
    } else {
        IndexReplayFailure::permanent_static(code)
    }
}

fn classify_mutation_failure(error: MutationStorageError) -> IndexReplayFailure {
    let (retryable, code) = match &error {
        MutationStorageError::Validation(_)
        | MutationStorageError::InvalidDelivery { .. }
        | MutationStorageError::DeliveryConflict
        | MutationStorageError::DeliveryRejected
        | MutationStorageError::InvalidStoredSourceVersion { .. }
        | MutationStorageError::SqliteSourceVersionOutOfRange { .. }
        | MutationStorageError::Serialization(_) => (false, "mutation_rejected"),
        MutationStorageError::DeliveryInProgress { .. }
        | MutationStorageError::Storage(_)
        | MutationStorageError::ConcurrentMutationConflict
        | MutationStorageError::InboxCompletionLost => (true, "mutation_storage_retryable"),
    };
    tracing::error!(
        error = ?error,
        replay_failure_code = code,
        replay_failure_retryable = retryable,
        "Index replay mutation persistence failed"
    );
    if retryable {
        IndexReplayFailure::retryable_static(code)
    } else {
        IndexReplayFailure::permanent_static(code)
    }
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexReplayFailure> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => {
            tracing::error!(?backend, "Index replay checkpoint backend is unsupported");
            Err(IndexReplayFailure::permanent_static(
                "checkpoint_backend_unsupported",
            ))
        }
    }
}

fn checkpoint_storage_failure(
    code: &'static str,
    error: impl std::fmt::Display,
) -> IndexReplayFailure {
    tracing::error!(
        error = %error,
        replay_failure_code = code,
        "Index replay checkpoint storage failed"
    );
    IndexReplayFailure::retryable_static(code)
}

fn checkpoint_contract_failure(
    code: &'static str,
    error: impl std::fmt::Display,
) -> IndexReplayFailure {
    tracing::error!(
        error = %error,
        replay_failure_code = code,
        "Index replay checkpoint contract failed"
    );
    IndexReplayFailure::permanent_static(code)
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn uuid_value(value: uuid::Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn optional_source_version_value(
    source_version: Option<u64>,
    backend: DbBackend,
) -> Result<SqlValue, IndexReplayFailure> {
    match backend {
        DbBackend::Postgres => Ok(source_version.map(Decimal::from).into()),
        DbBackend::Sqlite => source_version
            .map(|value| {
                i64::try_from(value).map_err(|error| {
                    checkpoint_contract_failure("checkpoint_source_version_invalid", error)
                })
            })
            .transpose()
            .map(Into::into),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn checkpoint_key_values(
    key: &IndexReplayCheckpointKey,
    backend: DbBackend,
) -> Vec<SqlValue> {
    vec![
        uuid_value(key.tenant_id(), backend),
        "rebuild".into(),
        key.source_name().to_owned().into(),
        key.schema().module.as_str().to_owned().into(),
        key.schema().entity.as_str().to_owned().into(),
        i64::from(key.schema().version.get()).into(),
        key.locale()
            .map(|locale| locale.as_str().to_owned())
            .unwrap_or_default()
            .into(),
        "".into(),
    ]
}

fn select_checkpoint_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "SELECT cursor, CAST(source_version AS TEXT) AS source_version_text, last_delivery_id FROM index_checkpoints WHERE tenant_id = {prefix}1 AND checkpoint_kind = {prefix}2 AND source_name = {prefix}3 AND module_name = {prefix}4 AND entity_name = {prefix}5 AND schema_version = {prefix}6 AND locale_key = {prefix}7 AND partition_key = {prefix}8 LIMIT 1"
    )
}

fn upsert_checkpoint_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "INSERT INTO index_checkpoints (tenant_id, checkpoint_kind, source_name, module_name, entity_name, schema_version, locale_key, partition_key, cursor, source_version, last_delivery_id) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, {prefix}5, {prefix}6, {prefix}7, {prefix}8, {prefix}9, {prefix}10, {prefix}11) ON CONFLICT (tenant_id, checkpoint_kind, source_name, module_name, entity_name, schema_version, locale_key, partition_key) DO UPDATE SET cursor = excluded.cursor, source_version = COALESCE(excluded.source_version, index_checkpoints.source_version), last_delivery_id = COALESCE(excluded.last_delivery_id, index_checkpoints.last_delivery_id), updated_at = CURRENT_TIMESTAMP"
    )
}
