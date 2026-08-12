use std::collections::BTreeMap;

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value as SqlValue,
};
use serde_json::Value as JsonValue;
use thiserror::Error;
use uuid::Uuid;

use crate::{PersistedSchemaReadinessFailure, SchemaFingerprint, SchemaRef, SchemaRegistry};

pub const MAX_INDEX_SCHEMA_READINESS_SCHEMAS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSchemaReadinessRequest {
    tenant_id: Uuid,
    schemas: Vec<SchemaRef>,
}

impl IndexSchemaReadinessRequest {
    pub fn new(
        tenant_id: Uuid,
        schemas: impl IntoIterator<Item = SchemaRef>,
    ) -> Result<Self, IndexSchemaReadinessError> {
        if tenant_id.is_nil() {
            return Err(IndexSchemaReadinessError::NilTenantId);
        }

        let mut schemas = schemas.into_iter().collect::<Vec<_>>();
        if schemas.is_empty() {
            return Err(IndexSchemaReadinessError::EmptySchemaSet);
        }
        if schemas.len() > MAX_INDEX_SCHEMA_READINESS_SCHEMAS {
            return Err(IndexSchemaReadinessError::TooManySchemas {
                count: schemas.len(),
                maximum: MAX_INDEX_SCHEMA_READINESS_SCHEMAS,
            });
        }

        schemas.sort();
        if let Some(pair) = schemas.windows(2).find(|pair| pair[0] == pair[1]) {
            return Err(IndexSchemaReadinessError::DuplicateSchema(pair[0].clone()));
        }

        Ok(Self { tenant_id, schemas })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schemas(&self) -> &[SchemaRef] {
        &self.schemas
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSchemaReadinessFailure {
    pub reference: SchemaRef,
    pub reason: PersistedSchemaReadinessFailure,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSchemaReadinessEntry {
    pub reference: SchemaRef,
    pub fingerprint: SchemaFingerprint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexSchemaReadinessReceipt {
    tenant_id: Uuid,
    schemas: Vec<IndexSchemaReadinessEntry>,
}

impl IndexSchemaReadinessReceipt {
    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn schemas(&self) -> &[IndexSchemaReadinessEntry] {
        &self.schemas
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IndexSchemaReadinessError {
    #[error("schema readiness tenant id must not be nil")]
    NilTenantId,
    #[error("schema readiness set must not be empty")]
    EmptySchemaSet,
    #[error("schema readiness set contains {count} schemas; maximum is {maximum}")]
    TooManySchemas { count: usize, maximum: usize },
    #[error("schema readiness set contains duplicate schema: {0}")]
    DuplicateSchema(SchemaRef),
    #[error("schema readiness registry does not contain requested schema: {0}")]
    SchemaNotInRegistry(SchemaRef),
    #[error("persisted schema readiness row is invalid")]
    InvalidStoredSchema,
    #[error("tenant Index schema readiness is incomplete")]
    NotReady {
        failures: Vec<IndexSchemaReadinessFailure>,
    },
    #[error("schema readiness storage operation failed")]
    Storage(String),
}

/// Bounded fail-closed readiness gate over the tenant-scoped `index_schemas` registry.
///
/// The gate never writes schema rows. Source-owned schema registration remains owned by
/// `PostgresSchemaRegistrationStore`. This type verifies one explicit, bounded schema set in one
/// storage statement so a consumer cutover cannot mistake runtime capability presence for persisted
/// tenant readiness.
#[derive(Clone)]
pub struct PostgresIndexSchemaReadinessStore {
    db: DatabaseConnection,
}

impl PostgresIndexSchemaReadinessStore {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn require(
        &self,
        request: &IndexSchemaReadinessRequest,
        registry: &SchemaRegistry,
    ) -> Result<IndexSchemaReadinessReceipt, IndexSchemaReadinessError> {
        let mut expected = BTreeMap::new();
        let mut receipt_entries = Vec::with_capacity(request.schemas.len());

        for reference in &request.schemas {
            let registered = registry
                .get(reference)
                .ok_or_else(|| IndexSchemaReadinessError::SchemaNotInRegistry(reference.clone()))?;
            let schema_json = serde_json::to_value(&registered.schema)
                .map_err(|error| IndexSchemaReadinessError::Storage(error.to_string()))?;
            let key = schema_key(reference);
            expected.insert(
                key,
                ExpectedSchema {
                    reference: reference.clone(),
                    fingerprint: registered.fingerprint,
                    schema_json,
                },
            );
            receipt_entries.push(IndexSchemaReadinessEntry {
                reference: reference.clone(),
                fingerprint: registered.fingerprint,
            });
        }

        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let rows = self
            .db
            .query_all(Statement::from_sql_and_values(
                backend,
                readiness_sql(backend, request.schemas.len()),
                readiness_values(request, backend),
            ))
            .await
            .map_err(storage_error)?;

        let mut stored = BTreeMap::new();
        for row in rows {
            let persisted = stored_schema(row)?;
            if stored.insert(persisted.key.clone(), persisted).is_some() {
                return Err(IndexSchemaReadinessError::InvalidStoredSchema);
            }
        }

        let mut failures = Vec::new();
        for (key, expected_schema) in &expected {
            let Some(persisted) = stored.get(key) else {
                failures.push(IndexSchemaReadinessFailure {
                    reference: expected_schema.reference.clone(),
                    reason: PersistedSchemaReadinessFailure::Missing,
                });
                continue;
            };

            let reason = if persisted.status != "active" {
                Some(PersistedSchemaReadinessFailure::Inactive)
            } else if persisted.fingerprint != expected_schema.fingerprint.to_string() {
                Some(PersistedSchemaReadinessFailure::FingerprintMismatch)
            } else if persisted.schema_json != expected_schema.schema_json {
                Some(PersistedSchemaReadinessFailure::ContractMismatch)
            } else {
                None
            };

            if let Some(reason) = reason {
                failures.push(IndexSchemaReadinessFailure {
                    reference: expected_schema.reference.clone(),
                    reason,
                });
            }
        }

        if !failures.is_empty() {
            return Err(IndexSchemaReadinessError::NotReady { failures });
        }

        Ok(IndexSchemaReadinessReceipt {
            tenant_id: request.tenant_id,
            schemas: receipt_entries,
        })
    }
}

#[derive(Debug)]
struct ExpectedSchema {
    reference: SchemaRef,
    fingerprint: SchemaFingerprint,
    schema_json: JsonValue,
}

#[derive(Debug)]
struct StoredSchema {
    key: (String, String, u32),
    fingerprint: String,
    schema_json: JsonValue,
    status: String,
}

fn stored_schema(row: QueryResult) -> Result<StoredSchema, IndexSchemaReadinessError> {
    let module_name: String = row.try_get("", "module_name").map_err(storage_error)?;
    let entity_name: String = row.try_get("", "entity_name").map_err(storage_error)?;
    let schema_version: i64 = row.try_get("", "schema_version").map_err(storage_error)?;
    let schema_version = u32::try_from(schema_version)
        .ok()
        .filter(|version| *version > 0)
        .ok_or(IndexSchemaReadinessError::InvalidStoredSchema)?;

    Ok(StoredSchema {
        key: (module_name, entity_name, schema_version),
        fingerprint: row
            .try_get("", "schema_fingerprint")
            .map_err(storage_error)?,
        schema_json: row.try_get("", "schema_json").map_err(storage_error)?,
        status: row.try_get("", "status").map_err(storage_error)?,
    })
}

fn schema_key(reference: &SchemaRef) -> (String, String, u32) {
    (
        reference.module.as_str().to_owned(),
        reference.entity.as_str().to_owned(),
        reference.version.get(),
    )
}

fn readiness_values(request: &IndexSchemaReadinessRequest, backend: DbBackend) -> Vec<SqlValue> {
    let mut values = Vec::with_capacity(1 + request.schemas.len() * 3);
    values.push(uuid_value(request.tenant_id, backend));
    for reference in &request.schemas {
        values.push(reference.module.as_str().to_owned().into());
        values.push(reference.entity.as_str().to_owned().into());
        values.push(i64::from(reference.version.get()).into());
    }
    values
}

fn readiness_sql(backend: DbBackend, schema_count: usize) -> String {
    let mut clauses = Vec::with_capacity(schema_count);
    for index in 0..schema_count {
        let first = 2 + index * 3;
        let second = first + 1;
        let third = first + 2;
        clauses.push(match backend {
            DbBackend::Postgres => format!(
                "(module_name = ${first} AND entity_name = ${second} AND schema_version = ${third})"
            ),
            DbBackend::Sqlite => format!(
                "(module_name = ?{first} AND entity_name = ?{second} AND schema_version = ?{third})"
            ),
            _ => unreachable!("unsupported backend rejected before SQL selection"),
        });
    }

    let tenant_placeholder = match backend {
        DbBackend::Postgres => "$1",
        DbBackend::Sqlite => "?1",
        _ => unreachable!("unsupported backend rejected before SQL selection"),
    };
    format!(
        "SELECT module_name, entity_name, schema_version, schema_fingerprint, schema_json, status FROM index_schemas WHERE tenant_id = {tenant_placeholder} AND ({}) ORDER BY module_name, entity_name, schema_version",
        clauses.join(" OR ")
    )
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    if backend == DbBackend::Sqlite {
        value.to_string().into()
    } else {
        value.into()
    }
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexSchemaReadinessError> {
    if matches!(backend, DbBackend::Postgres | DbBackend::Sqlite) {
        Ok(())
    } else {
        Err(IndexSchemaReadinessError::Storage(
            "only PostgreSQL and SQLite are supported".to_owned(),
        ))
    }
}

fn storage_error(error: impl std::fmt::Display) -> IndexSchemaReadinessError {
    IndexSchemaReadinessError::Storage(error.to_string())
}
