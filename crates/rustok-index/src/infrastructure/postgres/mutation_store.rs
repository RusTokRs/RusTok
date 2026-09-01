use rust_decimal::Decimal;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use crate::{IndexMutation, SchemaRegistry};

const MAX_SOURCE_NAME_BYTES: usize = 128;
const MAX_DELIVERY_ID_BYTES: usize = 191;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutationDelivery {
    source_name: String,
    delivery_id: String,
    mutation: IndexMutation,
}

impl MutationDelivery {
    pub fn new(
        source_name: impl Into<String>,
        delivery_id: impl Into<String>,
        mutation: IndexMutation,
    ) -> Result<Self, MutationStorageError> {
        let source_name = source_name.into();
        let delivery_id = delivery_id.into();
        validate_delivery_part("source name", &source_name, MAX_SOURCE_NAME_BYTES)?;
        validate_delivery_part("delivery id", &delivery_id, MAX_DELIVERY_ID_BYTES)?;
        Ok(Self {
            source_name,
            delivery_id,
            mutation,
        })
    }

    pub fn from_event(
        source_name: impl Into<String>,
        mutation: IndexMutation,
    ) -> Result<Self, MutationStorageError> {
        let delivery_id = mutation.event_id().to_string();
        Self::new(source_name, delivery_id, mutation)
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn delivery_id(&self) -> &str {
        &self.delivery_id
    }

    pub fn mutation(&self) -> &IndexMutation {
        &self.mutation
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MutationApplyOutcome {
    Applied {
        source_version: u64,
    },
    Duplicate {
        source_version: u64,
    },
    StaleIgnored {
        incoming_source_version: u64,
        current_source_version: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MutationStorageError {
    #[error(transparent)]
    Validation(#[from] crate::RecordValidationError),
    #[error("invalid mutation delivery {field}: {reason}")]
    InvalidDelivery {
        field: &'static str,
        reason: &'static str,
    },
    #[error("mutation delivery identity was reused for a different payload")]
    DeliveryConflict,
    #[error("mutation delivery is already in non-terminal state {state}")]
    DeliveryInProgress { state: String },
    #[error("mutation delivery was rejected previously")]
    DeliveryRejected,
    #[error("stored source version is invalid: {value}")]
    InvalidStoredSourceVersion { value: String },
    #[error("source version {source_version} exceeds the SQLite contract-test range")]
    SqliteSourceVersionOutOfRange { source_version: u64 },
    #[error("mutation payload serialization failed: {0}")]
    Serialization(String),
    #[error("mutation storage operation failed")]
    Storage(String),
    #[error("mutation lost entity-key serialization")]
    ConcurrentMutationConflict,
    #[error("mutation inbox completion lost ownership")]
    InboxCompletionLost,
}

#[derive(Clone)]
pub struct PostgresMutationStore {
    db: DatabaseConnection,
}

impl PostgresMutationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn apply(
        &self,
        registry: &SchemaRegistry,
        delivery: &MutationDelivery,
    ) -> Result<MutationApplyOutcome, MutationStorageError> {
        registry.validate_mutation(delivery.mutation())?;
        let payload_bytes = serde_json::to_vec(delivery.mutation())
            .map_err(|error| MutationStorageError::Serialization(error.to_string()))?;
        let payload_hash = hex::encode(Sha256::digest(payload_bytes));

        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = self
            .apply_in_transaction(&transaction, registry, delivery, &payload_hash)
            .await;

        match result {
            Ok(outcome) => {
                transaction.commit().await.map_err(storage_error)?;
                Ok(outcome)
            }
            Err(error) => {
                transaction.rollback().await.map_err(storage_error)?;
                Err(error)
            }
        }
    }

    async fn apply_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        registry: &SchemaRegistry,
        delivery: &MutationDelivery,
        payload_hash: &str,
    ) -> Result<MutationApplyOutcome, MutationStorageError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        let mutation = delivery.mutation();
        let key = mutation.key();
        let source_version = mutation.source_version();
        let locale_key = key
            .locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned());
        let mutation_kind = match mutation {
            IndexMutation::Upsert { .. } => "upsert",
            IndexMutation::Delete { .. } => "delete",
        };

        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                insert_inbox_sql(backend),
                vec![
                    uuid_value(key.tenant_id, backend),
                    delivery.source_name().to_owned().into(),
                    delivery.delivery_id().to_owned().into(),
                    mutation_kind.into(),
                    key.schema.module.as_str().to_owned().into(),
                    key.schema.entity.as_str().to_owned().into(),
                    i64::from(key.schema.version.get()).into(),
                    uuid_value(key.entity_id, backend),
                    locale_key.clone().into(),
                    source_version_value(source_version, backend)?,
                    payload_hash.to_owned().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;

        if inserted.rows_affected() == 0 {
            return self
                .resolve_existing_delivery(transaction, delivery, payload_hash, backend)
                .await;
        }

        self.lock_entity_key(transaction, mutation, backend).await?;
        let current_source_version = self
            .current_source_version(transaction, mutation, backend)
            .await?;
        if source_version <= current_source_version {
            self.complete_inbox(transaction, delivery, payload_hash, backend)
                .await?;
            return Ok(MutationApplyOutcome::StaleIgnored {
                incoming_source_version: source_version,
                current_source_version,
            });
        }

        let registered = registry.get(&key.schema).ok_or_else(|| {
            MutationStorageError::Storage(
                "validated mutation schema disappeared from the registry".to_owned(),
            )
        })?;
        let schema_fingerprint = registered.fingerprint.to_string();

        self.delete_existing_links(transaction, mutation, backend)
            .await?;
        match mutation {
            IndexMutation::Upsert { record, .. } => {
                let payload = serde_json::to_value(&record.fields)
                    .map_err(|error| MutationStorageError::Serialization(error.to_string()))?;
                let applied = self
                    .upsert_entity(
                        transaction,
                        mutation,
                        &schema_fingerprint,
                        Some(payload),
                        false,
                        backend,
                    )
                    .await?;
                if !applied {
                    return Err(MutationStorageError::ConcurrentMutationConflict);
                }
                self.insert_links(transaction, record, backend).await?;
            }
            IndexMutation::Delete { .. } => {
                let applied = self
                    .upsert_entity(
                        transaction,
                        mutation,
                        &schema_fingerprint,
                        None,
                        true,
                        backend,
                    )
                    .await?;
                if !applied {
                    return Err(MutationStorageError::ConcurrentMutationConflict);
                }
            }
        }
        self.complete_inbox(transaction, delivery, payload_hash, backend)
            .await?;
        Ok(MutationApplyOutcome::Applied { source_version })
    }

    async fn resolve_existing_delivery(
        &self,
        transaction: &DatabaseTransaction,
        delivery: &MutationDelivery,
        payload_hash: &str,
        backend: DbBackend,
    ) -> Result<MutationApplyOutcome, MutationStorageError> {
        let key = delivery.mutation().key();
        let existing = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                select_inbox_sql(backend),
                vec![
                    uuid_value(key.tenant_id, backend),
                    delivery.source_name().to_owned().into(),
                    delivery.delivery_id().to_owned().into(),
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or_else(|| {
                MutationStorageError::Storage(
                    "inbox conflict row disappeared before it could be read".to_owned(),
                )
            })?;

        let stored_hash: String = existing
            .try_get("", "payload_hash")
            .map_err(storage_error)?;
        let stored_kind: String = existing
            .try_get("", "mutation_kind")
            .map_err(storage_error)?;
        let stored_module: String = existing.try_get("", "module_name").map_err(storage_error)?;
        let stored_entity: String = existing.try_get("", "entity_name").map_err(storage_error)?;
        let stored_schema_version: i64 = existing
            .try_get("", "schema_version")
            .map_err(storage_error)?;
        let stored_entity_id = stored_uuid(&existing, "entity_id", backend)?;
        let stored_locale: String = existing.try_get("", "locale_key").map_err(storage_error)?;
        let stored_source_version = stored_source_version(&existing)?;
        let state: String = existing.try_get("", "state").map_err(storage_error)?;

        let mutation_kind = match delivery.mutation() {
            IndexMutation::Upsert { .. } => "upsert",
            IndexMutation::Delete { .. } => "delete",
        };
        let locale_key = key
            .locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned());
        if stored_hash != payload_hash
            || stored_kind != mutation_kind
            || stored_module != key.schema.module.as_str()
            || stored_entity != key.schema.entity.as_str()
            || stored_schema_version != i64::from(key.schema.version.get())
            || stored_entity_id != key.entity_id
            || stored_locale != locale_key
            || stored_source_version != delivery.mutation().source_version()
        {
            return Err(MutationStorageError::DeliveryConflict);
        }

        match state.as_str() {
            "applied" => Ok(MutationApplyOutcome::Duplicate {
                source_version: stored_source_version,
            }),
            "rejected" => Err(MutationStorageError::DeliveryRejected),
            _ => Err(MutationStorageError::DeliveryInProgress { state }),
        }
    }

    async fn lock_entity_key(
        &self,
        transaction: &DatabaseTransaction,
        mutation: &IndexMutation,
        backend: DbBackend,
    ) -> Result<(), MutationStorageError> {
        if backend == DbBackend::Sqlite {
            return Ok(());
        }
        let key = mutation.key();
        let locale_key = key
            .locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned());
        let lock_key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            key.tenant_id,
            key.schema.module.as_str(),
            key.schema.entity.as_str(),
            key.schema.version.get(),
            key.entity_id,
            locale_key,
        );
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
                vec![lock_key.into()],
            ))
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    async fn current_source_version(
        &self,
        transaction: &DatabaseTransaction,
        mutation: &IndexMutation,
        backend: DbBackend,
    ) -> Result<u64, MutationStorageError> {
        let key = mutation.key();
        let locale_key = key
            .locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned());
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                select_entity_version_sql(backend),
                vec![
                    uuid_value(key.tenant_id, backend),
                    key.schema.module.as_str().to_owned().into(),
                    key.schema.entity.as_str().to_owned().into(),
                    i64::from(key.schema.version.get()).into(),
                    uuid_value(key.entity_id, backend),
                    locale_key.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        row.as_ref().map_or(Ok(0), stored_source_version)
    }

    async fn delete_existing_links(
        &self,
        transaction: &DatabaseTransaction,
        mutation: &IndexMutation,
        backend: DbBackend,
    ) -> Result<(), MutationStorageError> {
        let key = mutation.key();
        let locale_key = key
            .locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned());
        transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                delete_links_sql(backend),
                vec![
                    uuid_value(key.tenant_id, backend),
                    key.schema.module.as_str().to_owned().into(),
                    key.schema.entity.as_str().to_owned().into(),
                    i64::from(key.schema.version.get()).into(),
                    uuid_value(key.entity_id, backend),
                    locale_key.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        Ok(())
    }

    async fn upsert_entity(
        &self,
        transaction: &DatabaseTransaction,
        mutation: &IndexMutation,
        schema_fingerprint: &str,
        payload: Option<JsonValue>,
        is_deleted: bool,
        backend: DbBackend,
    ) -> Result<bool, MutationStorageError> {
        let key = mutation.key();
        let locale_key = key
            .locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned());
        let payload = SqlValue::Json(payload.map(Box::new));
        let result = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                upsert_entity_sql(backend),
                vec![
                    uuid_value(key.tenant_id, backend),
                    key.schema.module.as_str().to_owned().into(),
                    key.schema.entity.as_str().to_owned().into(),
                    i64::from(key.schema.version.get()).into(),
                    uuid_value(key.entity_id, backend),
                    locale_key.into(),
                    source_version_value(mutation.source_version(), backend)?,
                    schema_fingerprint.to_owned().into(),
                    payload,
                    is_deleted.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        Ok(result.rows_affected() == 1)
    }

    async fn insert_links(
        &self,
        transaction: &DatabaseTransaction,
        record: &crate::IndexRecord,
        backend: DbBackend,
    ) -> Result<(), MutationStorageError> {
        let source_locale = record
            .key
            .locale
            .as_ref()
            .map_or_else(String::new, |locale| locale.as_str().to_owned());
        for link in &record.links {
            for (ordinal, target) in link.targets.iter().enumerate() {
                let target_locale = target
                    .locale
                    .as_ref()
                    .map_or_else(String::new, |locale| locale.as_str().to_owned());
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        insert_link_sql(backend),
                        vec![
                            uuid_value(record.key.tenant_id, backend),
                            record.key.schema.module.as_str().to_owned().into(),
                            record.key.schema.entity.as_str().to_owned().into(),
                            i64::from(record.key.schema.version.get()).into(),
                            uuid_value(record.key.entity_id, backend),
                            source_locale.clone().into(),
                            source_version_value(record.source_version, backend)?,
                            link.name.as_str().to_owned().into(),
                            i64::try_from(ordinal)
                                .map_err(|_| {
                                    MutationStorageError::Storage(
                                        "link ordinal exceeds database integer range".to_owned(),
                                    )
                                })?
                                .into(),
                            target.schema.module.as_str().to_owned().into(),
                            target.schema.entity.as_str().to_owned().into(),
                            i64::from(target.schema.version.get()).into(),
                            uuid_value(target.entity_id, backend),
                            target_locale.into(),
                        ],
                    ))
                    .await
                    .map_err(storage_error)?;
            }
        }
        Ok(())
    }

    async fn complete_inbox(
        &self,
        transaction: &DatabaseTransaction,
        delivery: &MutationDelivery,
        payload_hash: &str,
        backend: DbBackend,
    ) -> Result<(), MutationStorageError> {
        let key = delivery.mutation().key();
        let completed = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                complete_inbox_sql(backend),
                vec![
                    uuid_value(key.tenant_id, backend),
                    delivery.source_name().to_owned().into(),
                    delivery.delivery_id().to_owned().into(),
                    payload_hash.to_owned().into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        if completed.rows_affected() != 1 {
            return Err(MutationStorageError::InboxCompletionLost);
        }
        Ok(())
    }
}

fn validate_delivery_part(
    field: &'static str,
    value: &str,
    max_bytes: usize,
) -> Result<(), MutationStorageError> {
    if value.is_empty() {
        return Err(MutationStorageError::InvalidDelivery {
            field,
            reason: "must not be empty",
        });
    }
    if value.trim() != value {
        return Err(MutationStorageError::InvalidDelivery {
            field,
            reason: "must not contain leading or trailing whitespace",
        });
    }
    if value.len() > max_bytes {
        return Err(MutationStorageError::InvalidDelivery {
            field,
            reason: "exceeds the storage limit",
        });
    }
    if value.chars().any(char::is_control) {
        return Err(MutationStorageError::InvalidDelivery {
            field,
            reason: "must not contain control characters",
        });
    }
    Ok(())
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), MutationStorageError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        backend => Err(MutationStorageError::Storage(format!(
            "Index mutation storage does not support {backend:?}"
        ))),
}
}

fn storage_error(error: impl std::fmt::Display) -> MutationStorageError {
    MutationStorageError::Storage(error.to_string())
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn source_version_expression(backend: DbBackend, index: usize) -> String {
    format!("{}{index}", placeholder_prefix(backend))
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn source_version_value(
    source_version: u64,
    backend: DbBackend,
) -> Result<SqlValue, MutationStorageError> {
    match backend {
        DbBackend::Postgres => Ok(Decimal::from(source_version).into()),
        DbBackend::Sqlite => i64::try_from(source_version)
            .map(Into::into)
            .map_err(|_| MutationStorageError::SqliteSourceVersionOutOfRange { source_version }),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn stored_uuid(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, MutationStorageError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(storage_error)?;
            Uuid::parse_str(&value).map_err(storage_error)
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn stored_source_version(row: &sea_orm::QueryResult) -> Result<u64, MutationStorageError> {
    let value: String = row
        .try_get("", "source_version_text")
        .map_err(storage_error)?;
    value
        .parse()
        .map_err(|_| MutationStorageError::InvalidStoredSourceVersion { value })
}

fn insert_inbox_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let source_version = source_version_expression(backend, 10);
    format!(
        "INSERT INTO index_inbox (tenant_id, source_name, delivery_id, mutation_kind, module_name, entity_name, schema_version, entity_id, locale_key, source_version, payload_hash, state, attempt_count) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, {prefix}5, {prefix}6, {prefix}7, {prefix}8, {prefix}9, {source_version}, {prefix}11, 'pending', 1) ON CONFLICT (tenant_id, source_name, delivery_id) DO NOTHING"
    )
}

fn select_inbox_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "SELECT mutation_kind, module_name, entity_name, schema_version, entity_id, locale_key, CAST(source_version AS TEXT) AS source_version_text, payload_hash, state FROM index_inbox WHERE tenant_id = {prefix}1 AND source_name = {prefix}2 AND delivery_id = {prefix}3 LIMIT 1"
    )
}

fn select_entity_version_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let lock = match backend {
        DbBackend::Postgres => " FOR UPDATE",
        DbBackend::Sqlite => "",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT CAST(source_version AS TEXT) AS source_version_text FROM index_entities WHERE tenant_id = {prefix}1 AND module_name = {prefix}2 AND entity_name = {prefix}3 AND schema_version = {prefix}4 AND entity_id = {prefix}5 AND locale_key = {prefix}6{lock}"
    )
}

fn delete_links_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "DELETE FROM index_links WHERE tenant_id = {prefix}1 AND source_module = {prefix}2 AND source_entity = {prefix}3 AND source_schema_version = {prefix}4 AND source_entity_id = {prefix}5 AND source_locale_key = {prefix}6"
    )
}

fn upsert_entity_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let source_version = source_version_expression(backend, 7);
    format!(
        "INSERT INTO index_entities (tenant_id, module_name, entity_name, schema_version, entity_id, locale_key, source_version, schema_fingerprint, payload, is_deleted) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, {prefix}5, {prefix}6, {source_version}, {prefix}8, {prefix}9, {prefix}10) ON CONFLICT (tenant_id, module_name, entity_name, schema_version, entity_id, locale_key) DO UPDATE SET source_version = excluded.source_version, schema_fingerprint = excluded.schema_fingerprint, payload = excluded.payload, is_deleted = excluded.is_deleted, updated_at = CURRENT_TIMESTAMP WHERE excluded.source_version > index_entities.source_version"
    )
}

fn insert_link_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let source_version = source_version_expression(backend, 7);
    format!(
        "INSERT INTO index_links (tenant_id, source_module, source_entity, source_schema_version, source_entity_id, source_locale_key, source_version, link_name, ordinal, target_module, target_entity, target_schema_version, target_entity_id, target_locale_key) VALUES ({prefix}1, {prefix}2, {prefix}3, {prefix}4, {prefix}5, {prefix}6, {source_version}, {prefix}8, {prefix}9, {prefix}10, {prefix}11, {prefix}12, {prefix}13, {prefix}14)"
    )
}

fn complete_inbox_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    format!(
        "UPDATE index_inbox SET state = 'applied', completed_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, lease_owner = NULL, lease_expires_at = NULL, error_code = NULL, error_details = NULL WHERE tenant_id = {prefix}1 AND source_name = {prefix}2 AND delivery_id = {prefix}3 AND payload_hash = {prefix}4 AND state = 'pending'"
    )
}
