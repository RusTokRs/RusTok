use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::{IndexSchema, SchemaFingerprint, SchemaIdentity, SchemaRef, SchemaVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersistedSchemaRegistrationOutcome {
    Inserted {
        reference: SchemaRef,
        fingerprint: SchemaFingerprint,
    },
    Unchanged {
        reference: SchemaRef,
        fingerprint: SchemaFingerprint,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersistedSchemaSupersessionOutcome {
    registration: PersistedSchemaRegistrationOutcome,
    retired_schema_count: u64,
}

impl PersistedSchemaSupersessionOutcome {
    pub fn registration(&self) -> &PersistedSchemaRegistrationOutcome {
        &self.registration
    }

    pub fn retired_schema_count(&self) -> u64 {
        self.retired_schema_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SchemaRegistrationError {
    #[error("schema registration tenant id must not be nil")]
    NilTenantId,
    #[error("invalid Index schema registration contract: {0}")]
    InvalidSchema(String),
    #[error("schema {reference} is already registered with another contract")]
    VersionConflict { reference: SchemaRef },
    #[error(
        "schema version must increase for {identity}: latest is {latest:?}, attempted {attempted:?}"
    )]
    NonMonotonicVersion {
        identity: SchemaIdentity,
        latest: SchemaVersion,
        attempted: SchemaVersion,
    },
    #[error("schema is retired and cannot be registered again: {0}")]
    SchemaRetired(SchemaRef),
    #[error("schema registration storage operation failed")]
    Storage(String),
}

/// Index-owned tenant-scoped persistence for source-published schema contracts.
///
/// The store serializes one schema identity at a time, preserves exact-version
/// idempotency, rejects contract reuse and lower-version insertion, and never
/// imports source-domain types. Source adapters must use this API rather than
/// writing `index_schemas` directly.
#[derive(Clone)]
pub struct PostgresSchemaRegistrationStore {
    db: DatabaseConnection,
}

impl PostgresSchemaRegistrationStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn register(
        &self,
        tenant_id: Uuid,
        schema: &IndexSchema,
    ) -> Result<PersistedSchemaRegistrationOutcome, SchemaRegistrationError> {
        let (fingerprint, schema_json) = registration_contract(tenant_id, schema)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = self
            .register_in_transaction(&transaction, tenant_id, schema, fingerprint, &schema_json)
            .await;
        finish_transaction(transaction, result).await
    }

    /// Registers exactly one new/current schema contract for an identity and atomically retires every
    /// lower active persisted routing key for the same tenant/module/entity.
    ///
    /// This is an explicit single-current supersession primitive, not the default registration path.
    /// Historical entity/link/inbox/replay rows are not deleted or rewritten; their schema rows remain
    /// present for foreign-key integrity but become non-authoritative because persisted readiness and
    /// query execution reject retired contracts. Callers must still rebuild/materialize the new current
    /// routing key before any consumer cutover.
    pub async fn register_current(
        &self,
        tenant_id: Uuid,
        schema: &IndexSchema,
    ) -> Result<PersistedSchemaSupersessionOutcome, SchemaRegistrationError> {
        let (fingerprint, schema_json) = registration_contract(tenant_id, schema)?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = self
            .register_current_in_transaction(
                &transaction,
                tenant_id,
                schema,
                fingerprint,
                &schema_json,
            )
            .await;
        finish_transaction(transaction, result).await
    }

    async fn register_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        schema: &IndexSchema,
        fingerprint: SchemaFingerprint,
        schema_json: &JsonValue,
    ) -> Result<PersistedSchemaRegistrationOutcome, SchemaRegistrationError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        lock_schema_identity(transaction, tenant_id, schema, backend).await?;

        if let Some(existing) = load_exact_schema(transaction, tenant_id, schema, backend).await? {
            return resolve_existing_schema(schema, fingerprint, schema_json, existing);
        }

        if let Some(latest) = load_latest_version(transaction, tenant_id, schema, backend).await?
            && schema.reference.version <= latest
        {
            return Err(SchemaRegistrationError::NonMonotonicVersion {
                identity: schema.reference.identity(),
                latest,
                attempted: schema.reference.version,
            });
        }

        insert_or_resolve_schema(
            transaction,
            tenant_id,
            schema,
            fingerprint,
            schema_json,
            backend,
        )
        .await
    }

    async fn register_current_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        tenant_id: Uuid,
        schema: &IndexSchema,
        fingerprint: SchemaFingerprint,
        schema_json: &JsonValue,
    ) -> Result<PersistedSchemaSupersessionOutcome, SchemaRegistrationError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        lock_schema_identity(transaction, tenant_id, schema, backend).await?;

        let latest = load_latest_version(transaction, tenant_id, schema, backend).await?;
        if let Some(latest) = latest
            && schema.reference.version < latest
        {
            return Err(SchemaRegistrationError::NonMonotonicVersion {
                identity: schema.reference.identity(),
                latest,
                attempted: schema.reference.version,
            });
        }

        let registration = if let Some(existing) =
            load_exact_schema(transaction, tenant_id, schema, backend).await?
        {
            resolve_existing_schema(schema, fingerprint, schema_json, existing)?
        } else {
            if latest == Some(schema.reference.version) {
                return Err(SchemaRegistrationError::Storage(
                    "latest schema routing key exists but exact contract row is missing".to_owned(),
                ));
            }
            insert_or_resolve_schema(
                transaction,
                tenant_id,
                schema,
                fingerprint,
                schema_json,
                backend,
            )
            .await?
        };

        let retired_schema_count =
            retire_lower_active_schemas(transaction, tenant_id, schema, backend).await?;

        Ok(PersistedSchemaSupersessionOutcome {
            registration,
            retired_schema_count,
        })
    }
}

fn registration_contract(
    tenant_id: Uuid,
    schema: &IndexSchema,
) -> Result<(SchemaFingerprint, JsonValue), SchemaRegistrationError> {
    if tenant_id.is_nil() {
        return Err(SchemaRegistrationError::NilTenantId);
    }
    let fingerprint = schema
        .fingerprint()
        .map_err(|error| SchemaRegistrationError::InvalidSchema(error.to_string()))?;
    let schema_json = serde_json::to_value(schema)
        .map_err(|error| SchemaRegistrationError::InvalidSchema(error.to_string()))?;
    Ok((fingerprint, schema_json))
}

async fn finish_transaction<T>(
    transaction: DatabaseTransaction,
    result: Result<T, SchemaRegistrationError>,
) -> Result<T, SchemaRegistrationError> {
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

async fn insert_or_resolve_schema(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    schema: &IndexSchema,
    fingerprint: SchemaFingerprint,
    schema_json: &JsonValue,
    backend: DbBackend,
) -> Result<PersistedSchemaRegistrationOutcome, SchemaRegistrationError> {
    let inserted = transaction
        .execute(Statement::from_sql_and_values(
            backend,
            insert_schema_sql(backend),
            schema_values(tenant_id, schema, fingerprint, schema_json, backend),
        ))
        .await
        .map_err(storage_error)?;
    if inserted.rows_affected() == 1 {
        return Ok(PersistedSchemaRegistrationOutcome::Inserted {
            reference: schema.reference.clone(),
            fingerprint,
        });
    }

    let existing = load_exact_schema(transaction, tenant_id, schema, backend)
        .await?
        .ok_or_else(|| {
            SchemaRegistrationError::Storage(
                "schema insert lost conflict row before verification".to_owned(),
            )
        })?;
    resolve_existing_schema(schema, fingerprint, schema_json, existing)
}

struct StoredSchema {
    fingerprint: String,
    schema_json: JsonValue,
    status: String,
}

fn resolve_existing_schema(
    schema: &IndexSchema,
    fingerprint: SchemaFingerprint,
    schema_json: &JsonValue,
    existing: StoredSchema,
) -> Result<PersistedSchemaRegistrationOutcome, SchemaRegistrationError> {
    if existing.status == "retired" {
        return Err(SchemaRegistrationError::SchemaRetired(
            schema.reference.clone(),
        ));
    }
    if existing.status != "active" {
        return Err(SchemaRegistrationError::Storage(
            "stored schema has unsupported status".to_owned(),
        ));
    }
    if existing.fingerprint != fingerprint.to_string() || existing.schema_json != *schema_json {
        return Err(SchemaRegistrationError::VersionConflict {
            reference: schema.reference.clone(),
        });
    }
    Ok(PersistedSchemaRegistrationOutcome::Unchanged {
        reference: schema.reference.clone(),
        fingerprint,
    })
}

async fn lock_schema_identity(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    schema: &IndexSchema,
    backend: DbBackend,
) -> Result<(), SchemaRegistrationError> {
    if backend == DbBackend::Sqlite {
        return Ok(());
    }
    let lock_key = format!(
        "{}\u{1f}{}\u{1f}{}",
        tenant_id,
        schema.reference.module.as_str(),
        schema.reference.entity.as_str(),
    );
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![lock_key.into()],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn load_exact_schema(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    schema: &IndexSchema,
    backend: DbBackend,
) -> Result<Option<StoredSchema>, SchemaRegistrationError> {
    transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            select_exact_schema_sql(backend),
            schema_scope_values(tenant_id, schema, backend),
        ))
        .await
        .map_err(storage_error)?
        .map(stored_schema)
        .transpose()
}

async fn load_latest_version(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    schema: &IndexSchema,
    backend: DbBackend,
) -> Result<Option<SchemaVersion>, SchemaRegistrationError> {
    transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            select_latest_version_sql(backend),
            schema_identity_values(tenant_id, schema, backend),
        ))
        .await
        .map_err(storage_error)?
        .map(|row| {
            let version: i32 = row.try_get("", "schema_version").map_err(storage_error)?;
            let version = u32::try_from(version).map_err(|_| {
                SchemaRegistrationError::Storage(
                    "stored schema version is outside the supported range".to_owned(),
                )
            })?;
            if version == 0 {
                return Err(SchemaRegistrationError::Storage(
                    "stored schema version must be positive".to_owned(),
                ));
            }
            Ok(SchemaVersion::new(version))
        })
        .transpose()
}

async fn retire_lower_active_schemas(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    schema: &IndexSchema,
    backend: DbBackend,
) -> Result<u64, SchemaRegistrationError> {
    transaction
        .execute(Statement::from_sql_and_values(
            backend,
            retire_lower_active_schemas_sql(backend),
            schema_scope_values(tenant_id, schema, backend),
        ))
        .await
        .map(|result| result.rows_affected())
        .map_err(storage_error)
}

fn stored_schema(row: QueryResult) -> Result<StoredSchema, SchemaRegistrationError> {
    Ok(StoredSchema {
        fingerprint: row
            .try_get("", "schema_fingerprint")
            .map_err(storage_error)?,
        schema_json: row.try_get("", "schema_json").map_err(storage_error)?,
        status: row.try_get("", "status").map_err(storage_error)?,
    })
}

fn schema_scope_values(tenant_id: Uuid, schema: &IndexSchema, backend: DbBackend) -> Vec<SqlValue> {
    let mut values = schema_identity_values(tenant_id, schema, backend);
    values.push(i64::from(schema.reference.version.get()).into());
    values
}

fn schema_identity_values(
    tenant_id: Uuid,
    schema: &IndexSchema,
    backend: DbBackend,
) -> Vec<SqlValue> {
    vec![
        uuid_value(tenant_id, backend),
        schema.reference.module.as_str().to_owned().into(),
        schema.reference.entity.as_str().to_owned().into(),
    ]
}

fn schema_values(
    tenant_id: Uuid,
    schema: &IndexSchema,
    fingerprint: SchemaFingerprint,
    schema_json: &JsonValue,
    backend: DbBackend,
) -> Vec<SqlValue> {
    vec![
        uuid_value(tenant_id, backend),
        schema.reference.module.as_str().to_owned().into(),
        schema.reference.entity.as_str().to_owned().into(),
        i64::from(schema.reference.version.get()).into(),
        fingerprint.to_string().into(),
        SqlValue::Json(Some(Box::new(schema_json.clone()))),
    ]
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    if backend == DbBackend::Sqlite {
        value.to_string().into()
    } else {
        value.into()
    }
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), SchemaRegistrationError> {
    if matches!(backend, DbBackend::Postgres | DbBackend::Sqlite) {
        Ok(())
    } else {
        Err(SchemaRegistrationError::Storage(
            "only PostgreSQL and SQLite are supported".to_owned(),
        ))
    }
}

fn insert_schema_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json, status) VALUES ($1, $2, $3, $4, $5, $6, 'active') ON CONFLICT (tenant_id, module_name, entity_name, schema_version) DO NOTHING"
        }
        DbBackend::Sqlite => {
            "INSERT INTO index_schemas (tenant_id, module_name, entity_name, schema_version, schema_fingerprint, schema_json, status) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active') ON CONFLICT (tenant_id, module_name, entity_name, schema_version) DO NOTHING"
        }
        _ => unreachable!("unsupported backend rejected before SQL selection"),
    }
}

fn select_exact_schema_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "SELECT schema_fingerprint, schema_json, status FROM index_schemas WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4"
        }
        DbBackend::Sqlite => {
            "SELECT schema_fingerprint, schema_json, status FROM index_schemas WHERE tenant_id = ?1 AND module_name = ?2 AND entity_name = ?3 AND schema_version = ?4"
        }
        _ => unreachable!("unsupported backend rejected before SQL selection"),
    }
}

fn select_latest_version_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "SELECT schema_version FROM index_schemas WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 ORDER BY schema_version DESC LIMIT 1"
        }
        DbBackend::Sqlite => {
            "SELECT schema_version FROM index_schemas WHERE tenant_id = ?1 AND module_name = ?2 AND entity_name = ?3 ORDER BY schema_version DESC LIMIT 1"
        }
        _ => unreachable!("unsupported backend rejected before SQL selection"),
    }
}

fn retire_lower_active_schemas_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "UPDATE index_schemas SET status = 'retired', updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version < $4 AND status = 'active'"
        }
        DbBackend::Sqlite => {
            "UPDATE index_schemas SET status = 'retired', updated_at = CURRENT_TIMESTAMP WHERE tenant_id = ?1 AND module_name = ?2 AND entity_name = ?3 AND schema_version < ?4 AND status = 'active'"
        }
        _ => unreachable!("unsupported backend rejected before SQL selection"),
    }
}

fn storage_error(error: impl std::fmt::Display) -> SchemaRegistrationError {
    SchemaRegistrationError::Storage(error.to_string())
}
