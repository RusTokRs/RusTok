//! Artifact data key, value, and prefix validations.

use async_trait::async_trait;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::*;
use super::constants::*;
use super::error::*;
use super::traits::*;
use super::types::*;

pub(crate) fn validate_artifact_data_value(value: &Value) -> Result<(), ArtifactDataError> {
    let encoded_bytes = artifact_data_value_size(value)?;
    if encoded_bytes > MAX_ARTIFACT_DATA_VALUE_BYTES as u64 {
        return Err(ArtifactDataError::ValueTooLarge {
            limit: MAX_ARTIFACT_DATA_VALUE_BYTES,
            actual: usize::try_from(encoded_bytes).unwrap_or(usize::MAX),
        });
    }
    Ok(())
}

pub(crate) fn artifact_data_value_size(value: &Value) -> Result<u64, ArtifactDataError> {
    let encoded =
        serde_json::to_vec(value).map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
    u64::try_from(encoded.len())
        .map_err(|_| ArtifactDataError::Storage("artifact data value size overflow".to_string()))
}

pub(crate) fn validate_artifact_data_batch(batch: &ArtifactDataBatchWrite) -> Result<(), ArtifactDataError> {
    if batch.writes.is_empty() || batch.writes.len() > MAX_ARTIFACT_DATA_BATCH_SIZE {
        return Err(ArtifactDataError::InvalidBatch);
    }
    let mut keys = HashSet::with_capacity(batch.writes.len());
    let mut idempotency_keys = HashSet::with_capacity(batch.writes.len());
    for write in &batch.writes {
        validate_artifact_data_key(&write.key)?;
        validate_artifact_data_value(&write.value)?;
        if write.idempotency_key.is_nil() {
            return Err(ArtifactDataError::InvalidIdempotencyKey);
        }
        if !keys.insert(&write.key) || !idempotency_keys.insert(write.idempotency_key) {
            return Err(ArtifactDataError::InvalidBatch);
        }
    }
    Ok(())
}

pub(crate) fn validate_page_request(page: &ArtifactDataPageRequest) -> Result<(), ArtifactDataError> {
    validate_artifact_data_prefix(&page.prefix)?;
    if page.limit == 0 || page.limit > MAX_ARTIFACT_DATA_PAGE_SIZE {
        return Err(ArtifactDataError::InvalidPage);
    }
    if let Some(after_key) = &page.after_key {
        validate_artifact_data_key(after_key)?;
        if !after_key.starts_with(&page.prefix) {
            return Err(ArtifactDataError::InvalidPage);
        }
    }
    Ok(())
}

pub(crate) fn validate_artifact_data_index_query(
    query: &ArtifactDataIndexQuery,
) -> Result<String, ArtifactDataError> {
    if query.index.is_empty()
        || query.index.len() > 64
        || !query
            .index
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        || !matches!(
            &query.value,
            Value::String(_) | Value::Number(_) | Value::Bool(_)
        )
    {
        return Err(ArtifactDataError::InvalidIndexQuery);
    }
    validate_page_request(&query.page)?;
    let value = serde_json::to_string(&query.value)
        .map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
    if value.len() > MAX_ARTIFACT_DATA_INDEX_VALUE_BYTES {
        return Err(ArtifactDataError::InvalidIndexQuery);
    }
    Ok(value)
}

pub(crate) fn valid_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && !value.starts_with('_')
        && !value.ends_with('_')
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

pub(crate) fn valid_upgrade_hook_binding_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '_' | '-' | '.')
        })
}

pub(crate) fn prefixed_sha256_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

pub(crate) fn object_for_upload(
    upload: &ArtifactDataObjectUpload,
) -> Result<ArtifactDataObject, ArtifactDataError> {
    if upload.idempotency_key.is_nil() {
        return Err(ArtifactDataError::InvalidIdempotencyKey);
    }
    let size_bytes =
        u64::try_from(upload.data.len()).map_err(|_| ArtifactDataError::InvalidObject)?;
    let object = ArtifactDataObject {
        name: upload.name.clone(),
        content_type: upload.content_type.clone(),
        size_bytes,
        digest_sha256: format!("sha256:{}", hex::encode(Sha256::digest(&upload.data))),
        revision: 1,
    };
    object.validate()?;
    if upload.expected_revision == Some(0) {
        return Err(ArtifactDataError::RevisionConflict);
    }
    Ok(object)
}

/// Resolves a data-contract schema only from the descriptor persisted with the
/// exact injected installation. It accepts an admitted lifecycle state so an
/// owner can validate a target contract before activation; the separate data
/// authorizer remains responsible for operation lifecycle policy.
#[derive(Clone)]
pub struct SeaOrmArtifactDataSchemaValidator {
    db: DatabaseConnection,
    installation_id: Uuid,
    schema_validators: Arc<ArtifactSchemaValidatorCache>,
}

impl SeaOrmArtifactDataSchemaValidator {
    /// The host injects the exact immutable installation selected for this
    /// data broker. It is never derived from the module slug at validation
    /// time and never crosses the artifact capability boundary.
    pub fn new(db: DatabaseConnection, installation_id: Uuid) -> Self {
        Self {
            db,
            installation_id,
            schema_validators: Arc::new(ArtifactSchemaValidatorCache::default()),
        }
    }

    async fn data_contract_schema(
        &self,
        scope: &ArtifactDataScope,
    ) -> Result<(String, Value), ArtifactDataError> {
        scope.validate()?;
        if self.installation_id.is_nil() {
            return Err(ArtifactDataError::DataContractUnavailable);
        }
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        let placeholders = match backend {
            DbBackend::Postgres => ("$1", "$2", "$3", "$4", "$5"),
            _ => ("?1", "?2", "?3", "?4", "?5"),
        };
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT installation.descriptor \
                     FROM module_artifact_installations installation \
                     JOIN module_artifact_admissions admission \
                       ON admission.installation_id = installation.installation_id \
                     JOIN module_artifact_data_namespaces namespace \
                       ON namespace.tenant_id = {} AND namespace.data_owner_id = installation.data_owner_id \
                      AND namespace.namespace_instance_id = {} AND namespace.data_contract_digest = {} \
                     WHERE installation.installation_id = {} \
                       AND installation.data_owner_id = {} \
                       AND (installation.scope_kind = 'platform' OR installation.tenant_id = {}) \
                       AND admission.status IN ('admitted', 'installed', 'active', 'inactive')",
                    placeholders.2, placeholders.3, placeholders.4,
                    placeholders.0, placeholders.1, placeholders.2,
                ),
                vec![
                    uuid_value(self.installation_id, backend),
                    uuid_value(scope.data_owner_id, backend),
                    uuid_value(scope.tenant_id, backend),
                    uuid_value(scope.namespace_instance_id, backend),
                    scope.data_contract_digest.clone().into(),
                ],
            ))
            .await
            .map_err(storage_error)?
            .ok_or(ArtifactDataError::DataContractUnavailable)?;
        // A commit error can represent an unknown outcome, so retain the
        // owner-generated chunk for session reaping instead of deleting it.
        transaction.commit().await.map_err(storage_error)?;

        let descriptor_value: Value = row.try_get("", "descriptor").map_err(storage_error)?;
        let descriptor =
            serde_json::from_value::<crate::ModuleArtifactDescriptor>(descriptor_value)
                .map_err(|_| ArtifactDataError::DataContractUnavailable)?;
        descriptor
            .validate()
            .map_err(|_| ArtifactDataError::DataContractUnavailable)?;
        let contract = descriptor
            .persistence_contract
            .as_ref()
            .filter(|contract| contract.revision == scope.data_contract_revision)
            .ok_or(ArtifactDataError::DataContractUnavailable)?;
        if crate::promotion::digest_json(contract)
            .map_err(|error| ArtifactDataError::Storage(error.to_string()))?
            != scope.data_contract_digest
        {
            return Err(ArtifactDataError::DataContractUnavailable);
        }
        let schema = descriptor
            .schema_document(&contract.schema_digest)
            .cloned()
            .ok_or(ArtifactDataError::DataContractUnavailable)?;
        Ok((contract.schema_digest.clone(), schema))
    }
}

#[async_trait]
impl ArtifactDataSchemaValidator for SeaOrmArtifactDataSchemaValidator {
    async fn validate_data_value(
        &self,
        scope: &ArtifactDataScope,
        value: &Value,
    ) -> Result<(), ArtifactDataError> {
        let (schema_digest, schema) = self.data_contract_schema(scope).await?;
        self.schema_validators
            .validate(&schema_digest, &schema, value)
            .map_err(|error| match error {
                ArtifactSchemaValidationError::Compilation
                | ArtifactSchemaValidationError::CachePoisoned => {
                    ArtifactDataError::DataContractSchemaInvalid
                }
                ArtifactSchemaValidationError::Violation => {
                    ArtifactDataError::DataContractSchemaViolation
                }
            })
    }
}
