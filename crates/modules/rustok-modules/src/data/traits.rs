//! Artifact data broker, hook, authorizer, and validator traits.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::*;
use super::error::*;
use super::types::*;

pub trait ArtifactDataBroker: Send + Sync {
    async fn get(
        &self,
        scope: &ArtifactDataScope,
        key: &str,
    ) -> Result<Option<ArtifactDataRecord>, ArtifactDataError>;

    async fn put(
        &self,
        scope: &ArtifactDataScope,
        write: ArtifactDataWrite,
    ) -> Result<ArtifactDataRecord, ArtifactDataError>;

    async fn put_batch(
        &self,
        scope: &ArtifactDataScope,
        batch: ArtifactDataBatchWrite,
    ) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError>;

    async fn delete(
        &self,
        scope: &ArtifactDataScope,
        request: ArtifactDataDeleteRequest,
    ) -> Result<ArtifactDataDeleteResult, ArtifactDataError>;

    async fn list(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataPage, ArtifactDataError>;

    async fn query_index(
        &self,
        _scope: &ArtifactDataScope,
        _query: ArtifactDataIndexQuery,
    ) -> Result<ArtifactDataPage, ArtifactDataError> {
        Err(ArtifactDataError::IndexQueryUnavailable)
    }
}

/// Owner-owned broker for bounded binary artifact data. Its public contract
/// contains logical names and verified bytes only; storage keys are private
/// implementation details and never cross this boundary.
#[async_trait]
pub trait ArtifactDataObjectBroker: Send + Sync {
    async fn get_object(
        &self,
        scope: &ArtifactDataScope,
        name: &str,
    ) -> Result<Option<ArtifactDataObject>, ArtifactDataError>;

    async fn read_object(
        &self,
        scope: &ArtifactDataScope,
        name: &str,
    ) -> Result<Option<ArtifactDataObjectContent>, ArtifactDataError>;

    async fn put_object(
        &self,
        scope: &ArtifactDataScope,
        upload: ArtifactDataObjectUpload,
    ) -> Result<ArtifactDataObject, ArtifactDataError>;

    async fn delete_object(
        &self,
        scope: &ArtifactDataScope,
        request: ArtifactDataObjectDeleteRequest,
    ) -> Result<ArtifactDataObjectDeleteResult, ArtifactDataError>;

    async fn list_objects(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataObjectPage, ArtifactDataError>;
}


/// Policy evaluation is host-owned and request-scoped. The implementation can
/// bind actor, grants, quotas, and the admitted policy revision without giving
/// an artifact a direct handle to any of those systems.
#[async_trait]
pub trait ArtifactDataAuthorizer: Send + Sync {
    async fn authorize_data(
        &self,
        scope: &ArtifactDataScope,
        access: ArtifactDataAccess,
    ) -> Result<(), ArtifactDataError>;
}

/// The host resolves the admitted data-contract schema and validates a bounded
/// structured value before it becomes durable. Production adapters must use the
/// maintained `jsonschema` validator with bounded regular-expression settings;
/// artifacts never supply an executable validator or schema location.
#[async_trait]
pub trait ArtifactDataSchemaValidator: Send + Sync {
    async fn validate_data_value(
        &self,
        scope: &ArtifactDataScope,
        value: &Value,
    ) -> Result<(), ArtifactDataError>;
}


#[async_trait]
pub trait ArtifactDataPurgeAuthorizer: Send + Sync {
    async fn authorize_purge_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &ArtifactDataPurgeRequest,
        context: &ArtifactDataPurgeAuthorizationContext,
    ) -> Result<(), ArtifactDataError>;
}

/// Host-owned authorization for an operator data export. The sandbox has no
/// export capability: this port receives only a bounded owner request.
#[async_trait]
pub trait ArtifactDataExportAuthorizer: Send + Sync {
    async fn authorize_export(
        &self,
        request: &ArtifactDataExportRequest,
    ) -> Result<(), ArtifactDataError>;
}


/// Owner-owned installation checkpoint boundary. Implementations must retain
/// the installation revision CAS and transactional outbox semantics.
#[async_trait]
pub trait ArtifactDataMigrationCheckpointStore: Send + Sync {
    async fn record_data_upgrade_checkpoint(
        &self,
        request: ArtifactMigrationCheckpointRequest,
    ) -> Result<u64, ArtifactDataError>;
}
