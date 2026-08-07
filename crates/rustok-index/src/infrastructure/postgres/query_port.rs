use std::{collections::BTreeSet, sync::Arc};

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    application::{
        CompiledPostgresCell, CompiledPostgresCount, CompiledPostgresPageQuery,
        CompiledPostgresQuery, CompiledPostgresRow, CompiledQueryColumn,
        IndexQueryExecutionError, IndexQueryPage, IndexQueryPort,
        PersistedSchemaReadinessFailure, PostgresBindValue, SchemaRegistry,
    },
    domain::{IndexQuery, SchemaRef},
};

use super::PostgresIndexQueryAdmissionCatalog;

const EXACT_COUNT_ALIAS: &str = "__exact_count";
const READ_ONLY_SNAPSHOT_SQL: &str =
    "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY";
const SELECT_SCHEMA_READINESS_SQL: &str =
    "SELECT schema_fingerprint, schema_json, status FROM index_schemas WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4";

#[derive(Debug)]
struct RequiredSchemaContract {
    reference: SchemaRef,
    fingerprint: String,
    schema_json: JsonValue,
}

/// PostgreSQL execution adapter for the transport-neutral [`IndexQueryPort`].
///
/// The adapter compiles through the owned immutable registry, applies trusted owner entity admission
/// and any query-path-scoped generic link-target availability policy before filter/order/pagination/
/// count execution, verifies every schema touched by the plan against tenant-scoped persisted
/// registration, and executes the page plus optional exact count inside one read-only repeatable-read
/// snapshot.
#[derive(Clone)]
pub struct PostgresIndexQueryPort {
    db: DatabaseConnection,
    registry: Arc<SchemaRegistry>,
    admissions: PostgresIndexQueryAdmissionCatalog,
}

impl PostgresIndexQueryPort {
    pub fn new(db: DatabaseConnection, registry: Arc<SchemaRegistry>) -> Self {
        Self::with_admissions(db, registry, PostgresIndexQueryAdmissionCatalog::new())
    }

    pub fn with_admissions(
        db: DatabaseConnection,
        registry: Arc<SchemaRegistry>,
        admissions: PostgresIndexQueryAdmissionCatalog,
    ) -> Self {
        Self {
            db,
            registry,
            admissions,
        }
    }

    pub fn connection(&self) -> &DatabaseConnection {
        &self.db
    }

    pub fn registry(&self) -> &SchemaRegistry {
        &self.registry
    }

    pub fn admissions(&self) -> &PostgresIndexQueryAdmissionCatalog {
        &self.admissions
    }

    fn admitted_compiled_query(
        &self,
        query: &IndexQuery,
        page_query: &CompiledPostgresPageQuery,
    ) -> Result<CompiledPostgresQuery, IndexQueryExecutionError> {
        let mut compiled = page_query.compiled().clone();
        self.admissions
            .apply_link_target_availability(query, &mut compiled)
            .map_err(|error| {
                IndexQueryExecutionError::contract_preparation(query.schema.clone(), error)
            })?;
        if let Some(descriptor) = self.admissions.get(&query.schema) {
            descriptor
                .admission()
                .apply(&mut compiled)
                .map_err(|error| {
                    IndexQueryExecutionError::contract_preparation(query.schema.clone(), error)
                })?;
        }
        Ok(compiled)
    }

    async fn execute_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        query: &IndexQuery,
        page_query: &CompiledPostgresPageQuery,
        compiled: &CompiledPostgresQuery,
        required_schemas: &[RequiredSchemaContract],
    ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
        transaction
            .execute_unprepared(READ_ONLY_SNAPSHOT_SQL)
            .await
            .map_err(|error| {
                IndexQueryExecutionError::storage("configure read-only snapshot", error)
            })?;

        verify_persisted_schemas(transaction, query, required_schemas).await?;

        let page_rows = transaction
            .query_all(compiled_statement(compiled))
            .await
            .map_err(|error| IndexQueryExecutionError::storage("execute page statement", error))?
            .into_iter()
            .map(|row| map_page_row(row, compiled))
            .collect::<Result<Vec<_>, _>>()?;

        let exact_count_row = match compiled.exact_count.as_ref() {
            Some(count) => Some(execute_exact_count(transaction, count).await?),
            None => None,
        };

        self.registry
            .decode_postgres_query_page(query, page_query, page_rows, exact_count_row)
            .map_err(IndexQueryExecutionError::from)
    }
}

#[async_trait]
impl IndexQueryPort for PostgresIndexQueryPort {
    async fn execute_query(
        &self,
        query: IndexQuery,
    ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(IndexQueryExecutionError::UnsupportedBackend);
        }

        let page_query = self.registry.compile_postgres_page_query(&query)?;
        let compiled = self.admitted_compiled_query(&query, &page_query)?;
        let required_schemas = required_schema_contracts(&self.registry, &query)?;
        let transaction = self.db.begin().await.map_err(|error| {
            IndexQueryExecutionError::storage("begin read-only snapshot", error)
        })?;
        let outcome = self
            .execute_in_transaction(
                &transaction,
                &query,
                &page_query,
                &compiled,
                &required_schemas,
            )
            .await;
        match outcome {
            Ok(page) => {
                transaction.commit().await.map_err(|error| {
                    IndexQueryExecutionError::storage("commit read-only snapshot", error)
                })?;
                Ok(page)
            }
            Err(error) => {
                let _ = transaction.rollback().await;
                Err(error)
            }
        }
    }
}

fn required_schema_contracts(
    registry: &SchemaRegistry,
    query: &IndexQuery,
) -> Result<Vec<RequiredSchemaContract>, IndexQueryExecutionError> {
    let plan = registry.plan(query)?;
    let mut references = BTreeSet::new();
    references.insert(plan.root_schema.clone());
    for join in &plan.joins {
        references.insert(join.source_schema.clone());
        references.insert(join.target_schema.clone());
    }

    references
        .into_iter()
        .map(|reference| {
            let registered = registry.get(&reference).ok_or_else(|| {
                IndexQueryExecutionError::contract_preparation(
                    query.schema.clone(),
                    format!("planned schema {reference} is absent from the immutable registry"),
                )
            })?;
            let schema_json = serde_json::to_value(&registered.schema).map_err(|error| {
                IndexQueryExecutionError::contract_preparation(query.schema.clone(), error)
            })?;
            Ok(RequiredSchemaContract {
                reference,
                fingerprint: registered.fingerprint.to_hex(),
                schema_json,
            })
        })
        .collect()
}

async fn verify_persisted_schemas(
    transaction: &DatabaseTransaction,
    query: &IndexQuery,
    required_schemas: &[RequiredSchemaContract],
) -> Result<(), IndexQueryExecutionError> {
    for required in required_schemas {
        let row = transaction
            .query_one(Statement::from_sql_and_values(
                DbBackend::Postgres,
                SELECT_SCHEMA_READINESS_SQL,
                vec![
                    query.scope.tenant_id.into(),
                    required.reference.module.as_str().to_owned().into(),
                    required.reference.entity.as_str().to_owned().into(),
                    i64::from(required.reference.version.get()).into(),
                ],
            ))
            .await
            .map_err(|error| {
                IndexQueryExecutionError::storage("verify persisted schema readiness", error)
            })?;
        let Some(row) = row else {
            return Err(IndexQueryExecutionError::PersistedSchemaNotReady {
                schema: required.reference.clone(),
                failure: PersistedSchemaReadinessFailure::NotRegistered,
            });
        };
        let status: String = row.try_get("", "status").map_err(|error| {
            IndexQueryExecutionError::storage("decode persisted schema status", error)
        })?;
        if status != "active" {
            return Err(IndexQueryExecutionError::PersistedSchemaNotReady {
                schema: required.reference.clone(),
                failure: PersistedSchemaReadinessFailure::Retired,
            });
        }
        let fingerprint: String = row.try_get("", "schema_fingerprint").map_err(|error| {
            IndexQueryExecutionError::storage("decode persisted schema fingerprint", error)
        })?;
        let schema_json: JsonValue = row.try_get("", "schema_json").map_err(|error| {
            IndexQueryExecutionError::storage("decode persisted schema JSON", error)
        })?;
        if fingerprint != required.fingerprint || schema_json != required.schema_json {
            return Err(IndexQueryExecutionError::PersistedSchemaNotReady {
                schema: required.reference.clone(),
                failure: PersistedSchemaReadinessFailure::FingerprintMismatch,
            });
        }
    }
    Ok(())
}

fn compiled_statement(compiled: &CompiledPostgresQuery) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        compiled.sql.clone(),
        compiled.binds.iter().cloned().map(bind_value_to_sql),
    )
}

async fn execute_exact_count(
    transaction: &DatabaseTransaction,
    count: &CompiledPostgresCount,
) -> Result<CompiledPostgresCell, IndexQueryExecutionError> {
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            count.sql.clone(),
            count.binds.iter().cloned().map(bind_value_to_sql),
        ))
        .await
        .map_err(|error| IndexQueryExecutionError::storage("execute exact count", error))?
        .ok_or_else(|| IndexQueryExecutionError::MissingExactCountRow)?;
    let value: i64 = row.try_get("", EXACT_COUNT_ALIAS).map_err(|error| {
        IndexQueryExecutionError::storage("decode exact count row", error)
    })?;
    Ok(CompiledPostgresCell::Integer(value))
}

fn map_page_row(
    row: QueryResult,
    compiled: &CompiledPostgresQuery,
) -> Result<CompiledPostgresRow, IndexQueryExecutionError> {
    let mut output = CompiledPostgresRow::new();
    for column in &compiled.columns {
        let output_alias = match column {
            CompiledQueryColumn::EntityId { output_alias, .. }
            | CompiledQueryColumn::Field { output_alias, .. }
            | CompiledQueryColumn::OrderValue { output_alias, .. } => output_alias,
        };
        let value: JsonValue = row.try_get("", output_alias).map_err(|error| {
            IndexQueryExecutionError::storage("decode page projection", error)
        })?;
        output.insert(output_alias.clone(), CompiledPostgresCell::Json(value));
    }
    for column in &compiled.many_relations {
        let value: JsonValue = row.try_get("", &column.output_alias).map_err(|error| {
            IndexQueryExecutionError::storage("decode many relation projection", error)
        })?;
        output.insert(column.output_alias.clone(), CompiledPostgresCell::Json(value));
    }
    Ok(output)
}

fn bind_value_to_sql(value: PostgresBindValue) -> SqlValue {
    match value {
        PostgresBindValue::Boolean(value) => value.into(),
        PostgresBindValue::Integer(value) => value.into(),
        PostgresBindValue::Decimal(value) => value.into(),
        PostgresBindValue::Text(value) => value.into(),
        PostgresBindValue::Uuid(value) => value.into(),
        PostgresBindValue::Timestamp(value) => value.into(),
        PostgresBindValue::Json(value) => value.into(),
    }
}