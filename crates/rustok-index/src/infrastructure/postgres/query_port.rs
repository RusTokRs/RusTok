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
        CompiledPostgresCell, CompiledPostgresCount, CompiledPostgresLocalizedPageQuery,
        CompiledPostgresPageQuery, CompiledPostgresQuery, CompiledPostgresRow, CompiledQueryColumn,
        IndexQueryExecutionError, IndexQueryPage, IndexQueryPort, PersistedSchemaReadinessFailure,
        PostgresBindValue, SchemaRegistry,
    },
    domain::{IndexQuery, LocalizedEntityQuery, SchemaRef},
};

use super::PostgresIndexQueryAdmissionCatalog;

const EXACT_COUNT_ALIAS: &str = "__exact_count";
const READ_ONLY_SNAPSHOT_SQL: &str = "SET TRANSACTION ISOLATION LEVEL REPEATABLE READ, READ ONLY";
const SELECT_SCHEMA_READINESS_SQL: &str = "SELECT schema_fingerprint, schema_json, status FROM index_schemas WHERE tenant_id = $1 AND module_name = $2 AND entity_name = $3 AND schema_version = $4";

#[derive(Debug)]
struct RequiredSchemaContract {
    reference: SchemaRef,
    fingerprint: String,
    schema_json: JsonValue,
}

/// PostgreSQL execution adapter for the transport-neutral [`IndexQueryPort`].
///
/// The adapter compiles through the owned immutable registry, applies query-path-scoped generic
/// link-target availability and trusted owner entity admission before filter/order/pagination/count
/// execution, verifies every schema touched by the plan against tenant-scoped persisted registration,
/// and executes the page plus optional exact count inside one read-only repeatable-read snapshot.
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
        self.apply_admissions(query, &mut compiled)?;
        Ok(compiled)
    }

    fn admitted_localized_page_query(
        &self,
        query: &LocalizedEntityQuery,
        mut page_query: CompiledPostgresLocalizedPageQuery,
    ) -> Result<CompiledPostgresLocalizedPageQuery, IndexQueryExecutionError> {
        self.apply_admissions(&query.query, page_query.compiled_mut())?;
        Ok(page_query)
    }

    fn apply_admissions(
        &self,
        query: &IndexQuery,
        compiled: &mut CompiledPostgresQuery,
    ) -> Result<(), IndexQueryExecutionError> {
        self.admissions
            .apply_link_target_availability(query, compiled)
            .map_err(|error| {
                IndexQueryExecutionError::contract_preparation(query.schema.clone(), error)
            })?;
        if let Some(descriptor) = self.admissions.get(&query.schema) {
            descriptor.admission().apply(compiled).map_err(|error| {
                IndexQueryExecutionError::contract_preparation(query.schema.clone(), error)
            })?;
        }
        Ok(())
    }

    async fn configure_snapshot_and_verify(
        transaction: &DatabaseTransaction,
        query: &IndexQuery,
        required_schemas: &[RequiredSchemaContract],
    ) -> Result<(), IndexQueryExecutionError> {
        transaction
            .execute_unprepared(READ_ONLY_SNAPSHOT_SQL)
            .await
            .map_err(|error| {
                IndexQueryExecutionError::storage("configure read-only snapshot", error)
            })?;
        verify_persisted_schemas(transaction, query, required_schemas).await
    }

    async fn execute_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        query: &IndexQuery,
        page_query: &CompiledPostgresPageQuery,
        compiled: &CompiledPostgresQuery,
        required_schemas: &[RequiredSchemaContract],
    ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
        Self::configure_snapshot_and_verify(transaction, query, required_schemas).await?;

        let page_rows = execute_page_rows(transaction, compiled).await?;
        let exact_count_row = match compiled.exact_count.as_ref() {
            Some(count) => Some(execute_exact_count(transaction, count).await?),
            None => None,
        };

        self.registry
            .decode_postgres_query_page(query, page_query, page_rows, exact_count_row)
            .map_err(IndexQueryExecutionError::from)
    }

    async fn execute_localized_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        query: &LocalizedEntityQuery,
        page_query: &CompiledPostgresLocalizedPageQuery,
        required_schemas: &[RequiredSchemaContract],
    ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
        Self::configure_snapshot_and_verify(transaction, &query.query, required_schemas).await?;

        let compiled = page_query.compiled();
        let page_rows = execute_page_rows(transaction, compiled).await?;
        let exact_count_row = match compiled.exact_count.as_ref() {
            Some(count) => Some(execute_exact_count(transaction, count).await?),
            None => None,
        };

        self.registry
            .decode_postgres_localized_query_page(query, page_query, page_rows, exact_count_row)
            .map_err(IndexQueryExecutionError::from)
    }

    async fn finish_transaction(
        transaction: DatabaseTransaction,
        result: Result<IndexQueryPage, IndexQueryExecutionError>,
    ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
        match result {
            Ok(page) => {
                transaction.commit().await.map_err(|error| {
                    IndexQueryExecutionError::storage("commit query snapshot", error)
                })?;
                Ok(page)
            }
            Err(error) => {
                transaction.rollback().await.map_err(|rollback_error| {
                    IndexQueryExecutionError::storage("rollback query snapshot", rollback_error)
                })?;
                Err(error)
            }
        }
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

        let required_schemas = required_schema_contracts(&self.registry, &query)?;
        let page_query = self.registry.compile_postgres_page_query(&query)?;
        let compiled = self.admitted_compiled_query(&query, &page_query)?;
        let transaction =
            self.db.begin().await.map_err(|error| {
                IndexQueryExecutionError::storage("begin query snapshot", error)
            })?;
        let result = self
            .execute_in_transaction(
                &transaction,
                &query,
                &page_query,
                &compiled,
                &required_schemas,
            )
            .await;
        Self::finish_transaction(transaction, result).await
    }

    async fn execute_localized_query(
        &self,
        query: LocalizedEntityQuery,
    ) -> Result<IndexQueryPage, IndexQueryExecutionError> {
        if self.db.get_database_backend() != DbBackend::Postgres {
            return Err(IndexQueryExecutionError::UnsupportedBackend);
        }

        let required_schemas = required_schema_contracts(&self.registry, &query.query)?;
        let page_query = self
            .registry
            .compile_postgres_localized_page_query(&query)?;
        // Trusted owner/link admission is applied to every physical fold alias before the read-only
        // transaction begins. A malformed admission contract therefore cannot execute page/count SQL.
        let page_query = self.admitted_localized_page_query(&query, page_query)?;
        let transaction = self.db.begin().await.map_err(|error| {
            IndexQueryExecutionError::storage("begin localized query snapshot", error)
        })?;
        let result = self
            .execute_localized_in_transaction(&transaction, &query, &page_query, &required_schemas)
            .await;
        Self::finish_transaction(transaction, result).await
    }
}

fn required_schema_contracts(
    registry: &SchemaRegistry,
    query: &IndexQuery,
) -> Result<Vec<RequiredSchemaContract>, IndexQueryExecutionError> {
    let plan = registry.plan_query(query)?;
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
                IndexQueryExecutionError::SchemaNotReady {
                    reference: reference.clone(),
                    reason: PersistedSchemaReadinessFailure::Missing,
                }
            })?;
            let schema_json = serde_json::to_value(&registered.schema).map_err(|error| {
                IndexQueryExecutionError::contract_preparation(reference.clone(), error)
            })?;
            Ok(RequiredSchemaContract {
                reference,
                fingerprint: registered.fingerprint.to_string(),
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
                IndexQueryExecutionError::storage("load persisted schema readiness", error)
            })?
            .ok_or_else(|| IndexQueryExecutionError::SchemaNotReady {
                reference: required.reference.clone(),
                reason: PersistedSchemaReadinessFailure::Missing,
            })?;

        let fingerprint: String = row.try_get("", "schema_fingerprint").map_err(|error| {
            IndexQueryExecutionError::storage("decode persisted schema fingerprint", error)
        })?;
        let schema_json: JsonValue = row.try_get("", "schema_json").map_err(|error| {
            IndexQueryExecutionError::storage("decode persisted schema contract", error)
        })?;
        let status: String = row.try_get("", "status").map_err(|error| {
            IndexQueryExecutionError::storage("decode persisted schema status", error)
        })?;

        if status != "active" {
            return Err(IndexQueryExecutionError::SchemaNotReady {
                reference: required.reference.clone(),
                reason: PersistedSchemaReadinessFailure::Inactive,
            });
        }
        if fingerprint != required.fingerprint {
            return Err(IndexQueryExecutionError::SchemaNotReady {
                reference: required.reference.clone(),
                reason: PersistedSchemaReadinessFailure::FingerprintMismatch,
            });
        }
        if schema_json != required.schema_json {
            return Err(IndexQueryExecutionError::SchemaNotReady {
                reference: required.reference.clone(),
                reason: PersistedSchemaReadinessFailure::ContractMismatch,
            });
        }
    }
    Ok(())
}

async fn execute_page_rows(
    transaction: &DatabaseTransaction,
    compiled: &CompiledPostgresQuery,
) -> Result<Vec<CompiledPostgresRow>, IndexQueryExecutionError> {
    transaction
        .query_all(compiled_statement(compiled))
        .await
        .map_err(|error| IndexQueryExecutionError::storage("execute page statement", error))?
        .into_iter()
        .map(|row| map_page_row(row, compiled))
        .collect::<Result<Vec<_>, _>>()
}

fn compiled_statement(compiled: &CompiledPostgresQuery) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        compiled.sql.clone(),
        compiled
            .binds
            .iter()
            .cloned()
            .map(postgres_bind_value)
            .collect::<Vec<_>>(),
    )
}

fn count_statement(compiled: &CompiledPostgresCount) -> Statement {
    Statement::from_sql_and_values(
        DbBackend::Postgres,
        compiled.sql.clone(),
        compiled
            .binds
            .iter()
            .cloned()
            .map(postgres_bind_value)
            .collect::<Vec<_>>(),
    )
}

fn postgres_bind_value(value: PostgresBindValue) -> SqlValue {
    match value {
        PostgresBindValue::Boolean(value) => value.into(),
        PostgresBindValue::Integer(value) => value.into(),
        PostgresBindValue::Decimal(value) => value.into(),
        PostgresBindValue::Text(value) => value.into(),
        PostgresBindValue::Uuid(value) => value.into(),
        PostgresBindValue::Timestamp(value) => value.into(),
        PostgresBindValue::Json(value) => SqlValue::Json(Some(Box::new(value))),
    }
}

fn map_page_row(
    row: QueryResult,
    compiled: &CompiledPostgresQuery,
) -> Result<CompiledPostgresRow, IndexQueryExecutionError> {
    let mut values = Vec::with_capacity(compiled.columns.len() + compiled.many_relations.len());
    for column in &compiled.columns {
        match column {
            CompiledQueryColumn::EntityId { output_alias, .. } => {
                values.push((
                    output_alias.clone(),
                    optional_uuid_cell(&row, output_alias)?,
                ));
            }
            CompiledQueryColumn::Field { output_alias, .. }
            | CompiledQueryColumn::OrderValue { output_alias, .. } => {
                values.push((
                    output_alias.clone(),
                    optional_json_cell(&row, output_alias)?,
                ));
            }
        }
    }
    for column in &compiled.many_relations {
        values.push((
            column.output_alias.clone(),
            optional_json_cell(&row, &column.output_alias)?,
        ));
    }
    Ok(CompiledPostgresRow::from_values(values))
}

fn optional_uuid_cell(
    row: &QueryResult,
    alias: &str,
) -> Result<CompiledPostgresCell, IndexQueryExecutionError> {
    let value: Option<Uuid> = row.try_get("", alias).map_err(|error| {
        IndexQueryExecutionError::invalid_row_column(alias, "a UUID or SQL null", error)
    })?;
    Ok(value.map_or(CompiledPostgresCell::Null, CompiledPostgresCell::Uuid))
}

fn optional_json_cell(
    row: &QueryResult,
    alias: &str,
) -> Result<CompiledPostgresCell, IndexQueryExecutionError> {
    let value: Option<JsonValue> = row.try_get("", alias).map_err(|error| {
        IndexQueryExecutionError::invalid_row_column(alias, "JSONB or SQL null", error)
    })?;
    Ok(value.map_or(CompiledPostgresCell::Null, CompiledPostgresCell::Json))
}

async fn execute_exact_count(
    transaction: &DatabaseTransaction,
    count: &CompiledPostgresCount,
) -> Result<CompiledPostgresRow, IndexQueryExecutionError> {
    let row = transaction
        .query_one(count_statement(count))
        .await
        .map_err(|error| IndexQueryExecutionError::storage("execute exact-count statement", error))?
        .ok_or(IndexQueryExecutionError::MissingExactCountRow)?;
    let value: i64 = row.try_get("", EXACT_COUNT_ALIAS).map_err(|error| {
        IndexQueryExecutionError::invalid_row_column(
            EXACT_COUNT_ALIAS,
            "a non-null PostgreSQL bigint",
            error,
        )
    })?;
    Ok(CompiledPostgresRow::from_values([(
        EXACT_COUNT_ALIAS.to_owned(),
        CompiledPostgresCell::Integer(value),
    )]))
}
