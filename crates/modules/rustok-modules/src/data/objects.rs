//! SeaORM implementation of ArtifactDataObjectBroker.

use async_trait::async_trait;
use bytes::Bytes;
use object_store::path::Path;
use rustok_storage::{ObjectKey, ObjectScope, ObjectZone};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::constants::*;
use super::error::*;
use super::helpers::*;
use super::objects_persistence::*;
use super::traits::*;
use super::types::*;
use super::validation::*;
use super::*;

/// SeaORM and storage implementation of the private artifact object broker.
/// The generated path is intentionally not configurable through any artifact
/// command, metadata field, or capability payload.
#[derive(Clone)]
pub struct SeaOrmArtifactDataObjectBroker<A> {
    db: DatabaseConnection,
    storage: StorageRuntime,
    authorizer: A,
    infrastructure: ControlPlaneInfrastructure,
    quota: ArtifactDataQuota,
}

impl<A> SeaOrmArtifactDataObjectBroker<A>
where
    A: ArtifactDataAuthorizer,
{
    pub fn new(db: DatabaseConnection, storage: StorageRuntime, authorizer: A) -> Self {
        Self::with_infrastructure_and_quota(
            db,
            storage,
            authorizer,
            ControlPlaneInfrastructure::default(),
            ArtifactDataQuota::default(),
        )
    }

    pub fn with_infrastructure(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
    ) -> Self {
        Self::with_infrastructure_and_quota(
            db,
            storage,
            authorizer,
            infrastructure,
            ArtifactDataQuota::default(),
        )
    }

    pub fn with_infrastructure_and_quota(
        db: DatabaseConnection,
        storage: StorageRuntime,
        authorizer: A,
        infrastructure: ControlPlaneInfrastructure,
        quota: ArtifactDataQuota,
    ) -> Self {
        Self {
            db,
            storage,
            authorizer,
            infrastructure,
            quota,
        }
    }
}

#[async_trait]
impl<A> ArtifactDataObjectBroker for SeaOrmArtifactDataObjectBroker<A>
where
    A: ArtifactDataAuthorizer,
{
    async fn get_object(
        &self,
        scope: &ArtifactDataScope,
        name: &str,
    ) -> Result<Option<ArtifactDataObject>, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(name)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectRead {
                    name: name.to_owned(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let object = find_artifact_data_object(&transaction, scope, name).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(object.map(|stored| stored.object))
    }

    async fn read_object(
        &self,
        scope: &ArtifactDataScope,
        name: &str,
    ) -> Result<Option<ArtifactDataObjectContent>, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(name)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectRead {
                    name: name.to_owned(),
                },
            )
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let stored = find_artifact_data_object(&transaction, scope, name).await?;
        transaction.commit().await.map_err(storage_error)?;
        let Some(stored) = stored else {
            return Ok(None);
        };
        let data = self
            .storage
            .objects
            .get(&Path::from(stored.storage_key.as_str()))
            .await
            .map_err(storage_error)?
            .bytes()
            .await
            .map_err(storage_error)?;
        if u64::try_from(data.len()).ok() != Some(stored.object.size_bytes)
            || format!("sha256:{}", hex::encode(Sha256::digest(&data)))
                != stored.object.digest_sha256
        {
            return Err(ArtifactDataError::ObjectIntegrity);
        }
        Ok(Some(ArtifactDataObjectContent {
            object: stored.object,
            data,
        }))
    }

    async fn put_object(
        &self,
        scope: &ArtifactDataScope,
        upload: ArtifactDataObjectUpload,
    ) -> Result<ArtifactDataObject, ArtifactDataError> {
        scope.validate()?;
        let requested = object_for_upload(&upload)?;
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectWrite {
                    name: requested.name.clone(),
                },
            )
            .await?;

        if let Some(existing) = self
            .find_object_operation(scope, &upload, &requested)
            .await?
        {
            return Ok(existing);
        }

        let generated_key = ObjectKey::chronological(
            "module-artifact-data",
            ObjectZone::Objects,
            ObjectScope::Namespace {
                tenant_id: scope.tenant_id,
                owner_id: scope.data_owner_id,
                instance_id: scope.namespace_instance_id,
            },
            self.infrastructure.now(),
            self.infrastructure.new_id(),
            "bin",
        )
        .map_err(|error| ArtifactDataError::Storage(error.to_string()))?
        .to_string();
        self.storage
            .objects
            .put_opts(
                &Path::from(generated_key.as_str()),
                upload.data.clone().into(),
                self.storage.put_options(&requested.content_type),
            )
            .await
            .map_err(storage_error)?;
        let uploaded_path = generated_key;
        if upload.data.len() as u64 != requested.size_bytes {
            let _ = self
                .storage
                .objects
                .delete(&Path::from(uploaded_path.as_str()))
                .await;
            return Err(ArtifactDataError::ObjectIntegrity);
        }

        let transaction = match self.db.begin().await.map_err(storage_error) {
            Ok(transaction) => transaction,
            Err(error) => {
                let _ = self
                    .storage
                    .objects
                    .delete(&Path::from(uploaded_path.as_str()))
                    .await;
                return Err(error);
            }
        };
        if let Err(error) = configure_tenant_scope(&transaction, scope.tenant_id).await {
            let _ = self
                .storage
                .objects
                .delete(&Path::from(uploaded_path.as_str()))
                .await;
            return Err(error);
        }
        let stored = match persist_artifact_data_object(
            &transaction,
            &self.infrastructure,
            scope,
            &upload,
            &requested,
            &uploaded_path,
            self.quota,
        )
        .await
        {
            Ok(stored) => stored,
            Err(error) => {
                let _ = self
                    .storage
                    .objects
                    .delete(&Path::from(uploaded_path.as_str()))
                    .await;
                return Err(error);
            }
        };
        transaction.commit().await.map_err(storage_error)?;
        if stored.storage_key != uploaded_path {
            let _ = self
                .storage
                .objects
                .delete(&Path::from(uploaded_path))
                .await;
        }
        Ok(stored.object)
    }

    async fn delete_object(
        &self,
        scope: &ArtifactDataScope,
        request: ArtifactDataObjectDeleteRequest,
    ) -> Result<ArtifactDataObjectDeleteResult, ArtifactDataError> {
        scope.validate()?;
        validate_artifact_data_key(&request.name)?;
        if request.expected_revision == 0 || request.idempotency_key.is_nil() {
            return Err(ArtifactDataError::InvalidObject);
        }
        self.authorizer
            .authorize_data(
                scope,
                ArtifactDataAccess::ObjectDelete {
                    name: request.name.clone(),
                },
            )
            .await?;

        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        ensure_active_namespace(&transaction, scope, backend).await?;
        if let Some(result) =
            find_artifact_data_object_delete_operation(&transaction, scope, &request).await?
        {
            transaction.commit().await.map_err(storage_error)?;
            return Ok(result);
        }

        let stored = find_artifact_data_object(&transaction, scope, &request.name)
            .await?
            .ok_or(ArtifactDataError::RevisionConflict)?;
        if stored.object.revision != request.expected_revision {
            return Err(ArtifactDataError::RevisionConflict);
        }
        let deleted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_objects
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND object_name = {} AND revision = {}",
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
                    request.name.clone().into(),
                    revision_value(request.expected_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if deleted.rows_affected() != 1 {
            return Err(ArtifactDataError::RevisionConflict);
        }
        queue_artifact_data_object_gc_candidate(
            &transaction,
            &self.infrastructure,
            scope,
            &stored.storage_key,
        )
        .await?;
        persist_artifact_data_object_delete_operation(&transaction, scope, &request).await?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataObjectDeleteResult {
            name: request.name,
            deleted_revision: request.expected_revision,
        })
    }

    async fn list_objects(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataObjectPage, ArtifactDataError> {
        scope.validate()?;
        validate_page_request(&page)?;
        self.authorizer
            .authorize_data(scope, ArtifactDataAccess::ObjectList)
            .await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let limit = i64::from(page.limit) + 1;
        let prefix = format!("{}%", escape_like_prefix(&page.prefix));
        let (query, values) = match page.after_key {
            Some(after_name) => (
                format!(
                    "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key
                     FROM module_artifact_data_objects
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND object_name LIKE {} ESCAPE '\\' AND object_name > {}
                     ORDER BY object_name ASC LIMIT {}",
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
                    prefix.clone().into(),
                    after_name.into(),
                    limit.into(),
                ],
            ),
            None => (
                format!(
                    "SELECT object_name, content_type, size_bytes, digest_sha256, revision, storage_key
                     FROM module_artifact_data_objects
                     WHERE tenant_id = {} AND data_owner_id = {} AND namespace_instance_id = {}
                     AND object_name LIKE {} ESCAPE '\\'
                     ORDER BY object_name ASC LIMIT {}",
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
                    prefix.into(),
                    limit.into(),
                ],
            ),
        };
        let mut objects = transaction
            .query_all_raw(Statement::from_sql_and_values(backend, query, values))
            .await
            .map_err(storage_error)?
            .into_iter()
            .map(stored_artifact_data_object_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        transaction.commit().await.map_err(storage_error)?;
        let next_after_name = if objects.len() > page.limit as usize {
            objects.truncate(page.limit as usize);
            objects.last().map(|object| object.object.name.clone())
        } else {
            None
        };
        Ok(ArtifactDataObjectPage {
            objects: objects.into_iter().map(|stored| stored.object).collect(),
            next_after_name,
        })
    }
}

impl<A> SeaOrmArtifactDataObjectBroker<A>
where
    A: ArtifactDataAuthorizer,
{
    async fn find_object_operation(
        &self,
        scope: &ArtifactDataScope,
        upload: &ArtifactDataObjectUpload,
        requested: &ArtifactDataObject,
    ) -> Result<Option<ArtifactDataObject>, ArtifactDataError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        ensure_active_namespace(&transaction, scope, backend).await?;
        let operation =
            find_artifact_data_object_operation(&transaction, scope, upload.idempotency_key)
                .await?;
        transaction.commit().await.map_err(storage_error)?;
        let Some((stored, expected_revision)) = operation else {
            return Ok(None);
        };
        validate_object_operation(&stored, upload, requested, expected_revision)?;
        Ok(Some(stored.object))
    }
}
