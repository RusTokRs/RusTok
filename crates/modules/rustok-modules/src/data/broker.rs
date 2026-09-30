//! SeaORM implementation of ArtifactDataBroker.

use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use uuid::Uuid;

use crate::ArtifactDataIndexField;

use super::*;
use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::structured_persistence::*;
use super::traits::*;
use super::types::*;
use super::validation::*;

/// SeaORM adapter for the host-owned structured-value namespace. It never
/// accepts a guest-selected database object, SQL fragment, or storage path.
#[derive(Clone)]
pub struct SeaOrmArtifactDataBroker<A, V> {
    db: DatabaseConnection,
    authorizer: A,
    schema_validator: V,
    indexes: Vec<ArtifactDataIndexField>,
    index_contract_digest: Option<String>,
    quota: ArtifactDataQuota,
}

impl<A, V> SeaOrmArtifactDataBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    pub fn new(db: DatabaseConnection, authorizer: A, schema_validator: V) -> Self {
        Self::with_indexes_and_quota(
            db,
            authorizer,
            schema_validator,
            Vec::new(),
            ArtifactDataQuota::default(),
        )
    }

    pub fn with_indexes(
        db: DatabaseConnection,
        authorizer: A,
        schema_validator: V,
        indexes: Vec<ArtifactDataIndexField>,
    ) -> Self {
        Self::with_indexes_and_quota(
            db,
            authorizer,
            schema_validator,
            indexes,
            ArtifactDataQuota::default(),
        )
    }

    pub fn with_indexes_and_quota(
        db: DatabaseConnection,
        authorizer: A,
        schema_validator: V,
        indexes: Vec<ArtifactDataIndexField>,
        quota: ArtifactDataQuota,
    ) -> Self {
        let index_contract_digest = index_contract_digest(&indexes);
        Self {
            db,
            authorizer,
            schema_validator,
            indexes,
            index_contract_digest,
            quota,
        }
    }
}

#[async_trait]
impl<A, V> ArtifactDataBroker for SeaOrmArtifactDataBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    async fn get(
        &self,
        scope: &ArtifactDataScope,
        key: &str,
    ) -> Result<Option<ArtifactDataRecord>, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(key)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::Read {
                    key: key.to_owned(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {} AND data_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                scope_values(scope, backend, key)?,
            ))
            .await
            .map_err(storage_error)?;
        // A commit error can represent an unknown outcome, so retain the
        // owner-generated object for retention/GC reconciliation.
        transaction.commit().await.map_err(storage_error)?;
        row.map(record_from_row).transpose()
    }

    async fn put(
        &self,
        scope: &ArtifactDataScope,
        write: ArtifactDataWrite,
    ) -> Result<ArtifactDataRecord, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(&write.key)?;
        validate_artifact_data_value(&write.value)?;
        if write.idempotency_key.is_nil() {
            return Err(ArtifactDataError::InvalidIdempotencyKey);
        }
        self.schema_validator
            .validate_data_value(scope, &write.value)
            .await?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::Write {
                    key: write.key.clone(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let record = persist_artifact_data_write(
            &transaction,
            scope,
            write,
            &self.indexes,
            self.index_contract_digest.as_deref(),
            self.quota,
        )
        .await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(record)
    }

    async fn put_batch(
        &self,
        scope: &ArtifactDataScope,
        batch: ArtifactDataBatchWrite,
    ) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_batch(&batch)?;
        for write in &batch.writes {
            self.schema_validator
                .validate_data_value(scope, &write.value)
                .await?;
            self.authorizer
                .authorize_data(
                    scope,
                    ArtifactDataAccess::Write {
                        key: write.key.clone(),
                    },
                )
                .await?;
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let mut records = Vec::with_capacity(batch.writes.len());
        for write in batch.writes {
            records.push(
                persist_artifact_data_write(
                    &transaction,
                    scope,
                    write,
                    &self.indexes,
                    self.index_contract_digest.as_deref(),
                    self.quota,
                )
                .await?,
            );
        }
        transaction.commit().await.map_err(storage_error)?;
        Ok(records)
    }

    async fn delete(
        &self,
        scope: &ArtifactDataScope,
        request: ArtifactDataDeleteRequest,
    ) -> Result<ArtifactDataDeleteResult, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(&request.key)?;
        if request.expected_revision == 0 || request.idempotency_key.is_nil() {
            return Err(ArtifactDataError::InvalidIdempotencyKey);
        }
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::Delete {
                    key: request.key.clone(),
                },
            )
            .await?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        ensure_active_namespace(&transaction, scope, backend).await?;
        if let Some(result) =
            find_artifact_data_delete_operation(&transaction, scope, &request).await?
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(result);
        }

        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND data_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                scope_values(scope, backend, &request.key)?,
            ))
            .await
            .map_err(storage_error)?
            .ok_or(ArtifactDataError::RevisionConflict)?;
        let current = record_from_row(row)?;
        if current.revision != request.expected_revision {
            return Err(ArtifactDataError::RevisionConflict);
        }
        let deleted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND data_key = {} AND revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    request.key.clone().into(),
                    revision_value(request.expected_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if deleted.rows_affected() != 1 {
            return Err(ArtifactDataError::RevisionConflict);
        }
        delete_artifact_data_indexes(&transaction, scope, &request.key).await?;
        persist_artifact_data_delete_operation(&transaction, scope, &request).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataDeleteResult {
            key: request.key,
            deleted_revision: request.expected_revision,
        })
    }

    async fn list(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataPage, ArtifactDataError> {
        scope.validate()?;
        validate_page_request(&page)?;
        self.authorizer
            .authorize_data(scope, ArtifactDataAccess::List)
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let query_limit = i64::from(page.limit) + 1;
        let prefix_pattern = format!("{}%", escape_like_prefix(&page.prefix));
        let (query, values) = match page.after_key {
            Some(after_key) => (
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND data_key LIKE {} ESCAPE '\\' AND data_key > {}
                     ORDER BY data_key ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    prefix_pattern.clone().into(),
                    after_key.into(),
                    query_limit.into(),
                ],
            ),
            None => (
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND data_key LIKE {} ESCAPE '\\'
                     ORDER BY data_key ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    prefix_pattern.into(),
                    query_limit.into(),
                ],
            ),
        };
        let mut records = transaction
            .query_all_raw(Statement::from_sql_and_values(backend, query, values))
            .await
            .map_err(storage_error)?
            .into_iter()
            .map(record_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await.map_err(storage_error)?;
        let next_after_key = if records.len() > page.limit as usize {
            records.truncate(page.limit as usize);
            records.last().map(|record| record.key.clone())
        } else {
            None
        };
        Ok(ArtifactDataPage {
            records,
            next_after_key,
        })
    }

    async fn query_index(
        &self,
        scope: &ArtifactDataScope,
        query: ArtifactDataIndexQuery,
    ) -> Result<ArtifactDataPage, ArtifactDataError> {
        scope.validate()?;
        let index_value = validate_artifact_data_index_query(&query)?;
        let index = self
            .indexes
            .iter()
            .find(|index| index.name == query.index)
            .ok_or(ArtifactDataError::IndexQueryUnavailable)?;
        if !artifact_data_index_value_matches(&query.value, index.value_type) {
            return Err(ArtifactDataError::InvalidIndexQuery);
        }
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::Query {
                    index: query.index.clone(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        validate_artifact_data_index_contract(
            &transaction,
            scope,
            backend,
            self.index_contract_digest
                .as_deref()
                .ok_or(ArtifactDataError::IndexQueryUnavailable)?,
            false,
        )
        .await?;
        let query_limit = i64::from(query.page.limit) + 1;
        let prefix_pattern = format!("{}%", escape_like_prefix(&query.page.prefix));
        let (statement, values) = match query.page.after_key {
            Some(after_key) => (
                format!(
                    "SELECT data.data_key, data.value, data.revision
                     FROM module_artifact_data_indexes indexed
                     INNER JOIN module_artifact_data data
                       ON data.tenant_id = indexed.tenant_id
                      AND data.data_owner_id = indexed.data_owner_id
                      AND data.namespace_instance_id = indexed.namespace_instance_id
                      AND data.data_key = indexed.data_key
                     WHERE indexed.tenant_id = {} AND indexed.data_owner_id = {}
                       AND indexed.namespace_instance_id = {} AND indexed.index_name = {}
                       AND indexed.index_value = {} AND data.data_key LIKE {} ESCAPE '\\'
                       AND data.data_key > {} ORDER BY data.data_key ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    query.index.into(),
                    index_value.clone().into(),
                    prefix_pattern.clone().into(),
                    after_key.into(),
                    query_limit.into(),
                ],
            ),
            None => (
                format!(
                    "SELECT data.data_key, data.value, data.revision
                     FROM module_artifact_data_indexes indexed
                     INNER JOIN module_artifact_data data
                       ON data.tenant_id = indexed.tenant_id
                      AND data.data_owner_id = indexed.data_owner_id
                      AND data.namespace_instance_id = indexed.namespace_instance_id
                      AND data.data_key = indexed.data_key
                     WHERE indexed.tenant_id = {} AND indexed.data_owner_id = {}
                       AND indexed.namespace_instance_id = {} AND indexed.index_name = {}
                       AND indexed.index_value = {} AND data.data_key LIKE {} ESCAPE '\\'
                     ORDER BY data.data_key ASC LIMIT {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    query.index.into(),
                    index_value.into(),
                    prefix_pattern.into(),
                    query_limit.into(),
                ],
            ),
        };
        let mut records = transaction
            .query_all_raw(Statement::from_sql_and_values(backend, statement, values))
            .await
            .map_err(storage_error)?
            .into_iter()
            .map(record_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await.map_err(storage_error)?;
        let next_after_key = if records.len() > query.page.limit as usize {
            records.truncate(query.page.limit as usize);
            records.last().map(|record| record.key.clone())
        } else {
            None
        };
        Ok(ArtifactDataPage {
            records,
            next_after_key,
        })
    }
}
