//! Sandbox capability broker for structured artifact data.

use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use uuid::Uuid;

use super::*;
use super::broker::*;
use super::constants::*;
use super::error::*;
use super::traits::*;
use super::types::*;
use super::validation::*;

/// The `platform.data` adapter for one admitted artifact namespace. It is
/// injected into the neutral sandbox runtime and delegates all persistence,
/// policy, schema, and RLS enforcement to the owner data broker.
#[derive(Clone)]
pub struct SeaOrmArtifactDataCapabilityBroker<A, V> {
    data: SeaOrmArtifactDataBroker<A, V>,
    scope: ArtifactDataScope,
}

impl<A, V> SeaOrmArtifactDataCapabilityBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    pub fn new(
        db: DatabaseConnection,
        authorizer: A,
        schema_validator: V,
        scope: ArtifactDataScope,
        indexes: Vec<ArtifactDataIndexField>,
    ) -> Self {
        Self::with_quota(
            db,
            authorizer,
            schema_validator,
            scope,
            indexes,
            ArtifactDataQuota::default(),
        )
    }

    pub fn with_quota(
        db: DatabaseConnection,
        authorizer: A,
        schema_validator: V,
        scope: ArtifactDataScope,
        indexes: Vec<ArtifactDataIndexField>,
        quota: ArtifactDataQuota,
    ) -> Self {
        Self {
            data: SeaOrmArtifactDataBroker::with_indexes_and_quota(
                db,
                authorizer,
                schema_validator,
                indexes,
                quota,
            ),
            scope,
        }
    }
}

#[async_trait]
impl<A, V> CapabilityBroker for SeaOrmArtifactDataCapabilityBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    async fn invoke(
        &self,
        call: &CapabilityCall,
        _grant: &CapabilityGrant,
    ) -> SandboxResult<CapabilityResponse> {
        if call.capability.as_str() != "platform.data"
            || call.context.tenant_id != Some(self.scope.tenant_id)
            || !matches!(
                &call.subject,
                SandboxSubject::ModuleArtifact { slug, .. } if slug == &self.scope.module_slug
            )
        {
            return Err(SandboxError::CapabilityDenied(call.capability.clone()));
        }
        match decode_data_capability_call(call)? {
            DataCapabilityCall::Get { key } => {
                let record = self
                    .data
                    .get(&self.scope, &key)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "record": record }),
                })
            }
            DataCapabilityCall::Put { write } => {
                let record = self
                    .data
                    .put(&self.scope, write)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "record": record }),
                })
            }
            DataCapabilityCall::PutBatch { batch } => {
                let records = self
                    .data
                    .put_batch(&self.scope, batch)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "records": records }),
                })
            }
            DataCapabilityCall::Delete { request } => {
                let result = self
                    .data
                    .delete(&self.scope, request)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({ "deletion": result }),
                })
            }
            DataCapabilityCall::List { page } => {
                let page = self
                    .data
                    .list(&self.scope, page)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({
                        "records": page.records,
                        "next_after_key": page.next_after_key,
                    }),
                })
            }
            DataCapabilityCall::QueryIndex { query } => {
                let page = self
                    .data
                    .query_index(&self.scope, query)
                    .await
                    .map_err(|error| data_capability_error(&call.capability, error))?;
                Ok(CapabilityResponse {
                    output: json!({
                        "records": page.records,
                        "next_after_key": page.next_after_key,
                    }),
                })
            }
        }
    }
}

/// Production resolver for the `platform.data` owner. It derives the complete
/// namespace from the exact sandbox installation identity and never accepts
/// tenant, module, revision, or schema information from an artifact call.
#[derive(Clone)]
pub struct SeaOrmArtifactDataCapabilityBrokerResolver {
    db: DatabaseConnection,
    quota_policy: Arc<dyn ArtifactDataQuotaPolicy>,
}

impl SeaOrmArtifactDataCapabilityBrokerResolver {
    pub fn new(db: DatabaseConnection) -> Self {
        Self::with_quota_policy(db, Arc::new(FixedArtifactDataQuotaPolicy::default()))
    }

    pub fn with_quota_policy(
        db: DatabaseConnection,
        quota_policy: Arc<dyn ArtifactDataQuotaPolicy>,
    ) -> Self {
        Self { db, quota_policy }
    }
}

#[derive(Clone)]
pub(crate) struct ExactArtifactDataAuthorizer {
    pub(crate) scope: ArtifactDataScope,
}

#[async_trait]
impl ArtifactDataAuthorizer for ExactArtifactDataAuthorizer {
    async fn authorize_data(
        &self,
        scope: &ArtifactDataScope,
        _access: ArtifactDataAccess,
    ) -> Result<(), ArtifactDataError> {
        if scope == &self.scope {
            Ok(())
        } else {
            Err(ArtifactDataError::PolicyDenied)
        }
    }
}

#[async_trait]
impl ArtifactCapabilityBrokerResolver for SeaOrmArtifactDataCapabilityBrokerResolver {
    async fn resolve_broker(
        &self,
        execution: &ArtifactCapabilityExecution,
        capability: &rustok_sandbox::CapabilityName,
    ) -> SandboxResult<Arc<dyn CapabilityBroker>> {
        if capability.as_str() != "platform.data" {
            return Err(SandboxError::CapabilityDenied(capability.clone()));
        }
        let installation =
            resolve_granted_artifact_capability(&self.db, execution, capability).await?;
        let scope = artifact_data_scope_for_execution(&installation, execution, capability)?;
        let scope = resolve_serving_artifact_data_scope(&self.db, &installation, scope)
            .await
            .map_err(|error| data_capability_error(capability, error))?;
        let quota = self
            .quota_policy
            .quota_for(&scope)
            .await
            .map_err(|error| data_capability_error(capability, error))?;
        let indexes = installation
            .descriptor
            .persistence_contract
            .as_ref()
            .map(|contract| contract.indexes.clone())
            .ok_or_else(|| SandboxError::CapabilityDenied(capability.clone()))?;
        let authorizer = ExactArtifactDataAuthorizer {
            scope: scope.clone(),
        };
        let schema_validator =
            SeaOrmArtifactDataSchemaValidator::new(self.db.clone(), execution.installation_id);
        Ok(Arc::new(SeaOrmArtifactDataCapabilityBroker::with_quota(
            self.db.clone(),
            authorizer,
            schema_validator,
            scope,
            indexes,
            quota,
        )))
    }
}


pub(crate) enum DataCapabilityCall {

    Get { key: String },
    Put { write: ArtifactDataWrite },
    PutBatch { batch: ArtifactDataBatchWrite },
    Delete { request: ArtifactDataDeleteRequest },
    List { page: ArtifactDataPageRequest },
    QueryIndex { query: ArtifactDataIndexQuery },
}

pub(crate) fn decode_data_capability_call(call: &CapabilityCall) -> SandboxResult<DataCapabilityCall> {
    let input = call
        .input
        .as_object()
        .ok_or_else(|| data_capability_constraint(call, "data input must be an object"))?;
    match call.operation.as_str() {
        "get" => {
            reject_data_capability_fields(call, input, &["key"])?;
            Ok(DataCapabilityCall::Get {
                key: required_data_capability_string(call, input, "key")?.to_string(),
            })
        }
        "put" => Ok(DataCapabilityCall::Put {
            write: decode_data_capability_write(call, input)?,
        }),
        "put_batch" => {
            reject_data_capability_fields(call, input, &["writes"])?;
            let writes = input
                .get("writes")
                .and_then(Value::as_array)
                .ok_or_else(|| data_capability_constraint(call, "data writes must be an array"))?
                .iter()
                .map(|value| {
                    value
                        .as_object()
                        .ok_or_else(|| {
                            data_capability_constraint(call, "data batch entry must be an object")
                        })
                        .and_then(|write| decode_data_capability_write(call, write))
                })
                .collect::<SandboxResult<Vec<_>>>()?;
            let batch = ArtifactDataBatchWrite { writes };
            validate_artifact_data_batch(&batch)
                .map_err(|_| data_capability_constraint(call, "data batch is invalid"))?;
            Ok(DataCapabilityCall::PutBatch { batch })
        }
        "delete" => {
            reject_data_capability_fields(
                call,
                input,
                &["key", "expected_revision", "idempotency_key"],
            )?;
            let expected_revision = input
                .get("expected_revision")
                .and_then(Value::as_u64)
                .filter(|revision| *revision > 0)
                .ok_or_else(|| {
                    data_capability_constraint(
                        call,
                        "data expected_revision must be a positive integer",
                    )
                })?;
            let idempotency_key = Uuid::parse_str(required_data_capability_string(
                call,
                input,
                "idempotency_key",
            )?)
            .map_err(|_| data_capability_constraint(call, "data idempotency_key must be a UUID"))?;
            Ok(DataCapabilityCall::Delete {
                request: ArtifactDataDeleteRequest {
                    key: required_data_capability_string(call, input, "key")?.to_string(),
                    expected_revision,
                    idempotency_key,
                },
            })
        }
        "list" => {
            reject_data_capability_fields(call, input, &["prefix", "after_key", "limit"])?;
            let prefix = required_data_capability_string(call, input, "prefix")?.to_string();
            let after_key = input
                .get("after_key")
                .map(|value| {
                    value.as_str().map(str::to_string).ok_or_else(|| {
                        data_capability_constraint(call, "data after_key must be a string")
                    })
                })
                .transpose()?;
            let limit = input
                .get("limit")
                .and_then(Value::as_u64)
                .filter(|limit| (1..=100).contains(limit))
                .ok_or_else(|| {
                    data_capability_constraint(call, "data list limit must be between 1 and 100")
                })?;
            let page = ArtifactDataPageRequest {
                prefix,
                after_key,
                limit: u32::try_from(limit).map_err(|_| {
                    data_capability_constraint(call, "data list limit must fit u32")
                })?,
            };
            validate_page_request(&page)
                .map_err(|_| data_capability_constraint(call, "data list page is invalid"))?;
            Ok(DataCapabilityCall::List { page })
        }
        "query_index" => {
            reject_data_capability_fields(
                call,
                input,
                &["index", "value", "prefix", "after_key", "limit"],
            )?;
            let after_key = input
                .get("after_key")
                .map(|value| {
                    value.as_str().map(str::to_string).ok_or_else(|| {
                        data_capability_constraint(call, "data after_key must be a string")
                    })
                })
                .transpose()?;
            let limit = input
                .get("limit")
                .and_then(Value::as_u64)
                .filter(|limit| (1..=100).contains(limit))
                .ok_or_else(|| {
                    data_capability_constraint(call, "data query limit must be between 1 and 100")
                })?;
            let query = ArtifactDataIndexQuery {
                index: required_data_capability_string(call, input, "index")?.to_string(),
                value: input
                    .get("value")
                    .cloned()
                    .ok_or_else(|| data_capability_constraint(call, "data query requires value"))?,
                page: ArtifactDataPageRequest {
                    prefix: required_data_capability_string(call, input, "prefix")?.to_string(),
                    after_key,
                    limit: u32::try_from(limit).map_err(|_| {
                        data_capability_constraint(call, "data query limit must fit u32")
                    })?,
                },
            };
            validate_artifact_data_index_query(&query)
                .map_err(|_| data_capability_constraint(call, "data index query is invalid"))?;
            Ok(DataCapabilityCall::QueryIndex { query })
        }
        _ => Err(data_capability_constraint(
            call,
            "data operation is unsupported",
        )),
    }
}

pub(crate) fn decode_data_capability_write(
    call: &CapabilityCall,
    input: &serde_json::Map<String, Value>,
) -> SandboxResult<ArtifactDataWrite> {
    reject_data_capability_fields(
        call,
        input,
        &["key", "value", "expected_revision", "idempotency_key"],
    )?;
    let value = input
        .get("value")
        .cloned()
        .ok_or_else(|| data_capability_constraint(call, "data put input requires value"))?;
    let expected_revision = input
        .get("expected_revision")
        .map(|value| {
            value
                .as_u64()
                .filter(|revision| *revision > 0)
                .ok_or_else(|| {
                    data_capability_constraint(
                        call,
                        "data expected_revision must be a positive integer",
                    )
                })
        })
        .transpose()?;
    let idempotency_key = Uuid::parse_str(required_data_capability_string(
        call,
        input,
        "idempotency_key",
    )?)
    .map_err(|_| data_capability_constraint(call, "data idempotency_key must be a UUID"))?;
    Ok(ArtifactDataWrite {
        key: required_data_capability_string(call, input, "key")?.to_string(),
        value,
        expected_revision,
        create_only: false,
        idempotency_key,
    })
}

pub(crate) fn data_capability_constraint(call: &CapabilityCall, reason: &str) -> SandboxError {
    SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: reason.to_string(),
    }
}

pub(crate) fn required_data_capability_string<'a>(
    call: &CapabilityCall,
    input: &'a serde_json::Map<String, Value>,
    field: &str,
) -> SandboxResult<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| data_capability_constraint(call, &format!("data {field} must be a string")))
}

pub(crate) fn reject_data_capability_fields(
    call: &CapabilityCall,
    input: &serde_json::Map<String, Value>,
    allowed: &[&str],
) -> SandboxResult<()> {
    if input.keys().any(|field| !allowed.contains(&field.as_str())) {
        return Err(data_capability_constraint(
            call,
            "data input contains an unsupported field",
        ));
    }
    Ok(())
}

pub(crate) fn escape_like_prefix(prefix: &str) -> String {
    prefix
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

pub(crate) fn data_capability_error(
    capability: &rustok_sandbox::CapabilityName,
    error: ArtifactDataError,
) -> SandboxError {
    match error {
        ArtifactDataError::InvalidScope
        | ArtifactDataError::InvalidKey
        | ArtifactDataError::InvalidObject
        | ArtifactDataError::InvalidPage
        | ArtifactDataError::InvalidIndexQuery
        | ArtifactDataError::IndexQueryUnavailable
        | ArtifactDataError::InvalidBatch
        | ArtifactDataError::RevisionConflict
        | ArtifactDataError::NamespacePurged
        | ArtifactDataError::PurgePrecondition
        | ArtifactDataError::NamespaceHeld
        | ArtifactDataError::ExportPrecondition
        | ArtifactDataError::SnapshotPrecondition
        | ArtifactDataError::SnapshotLimitExceeded
        | ArtifactDataError::RestorePrecondition
        | ArtifactDataError::SnapshotRetentionPrecondition
        | ArtifactDataError::SnapshotCollectionPrecondition
        | ArtifactDataError::InvalidIdempotencyKey
        | ArtifactDataError::IdempotencyConflict
        | ArtifactDataError::ValueTooLarge { .. }
        | ArtifactDataError::QuotaExceeded { .. }
        | ArtifactDataError::DataContractSchemaViolation
        | ArtifactDataError::PolicyDenied => SandboxError::CapabilityDenied(capability.clone()),
        ArtifactDataError::InvalidUpgrade
        | ArtifactDataError::UpgradeHook(_)
        | ArtifactDataError::StaleUpgradePlan
        | ArtifactDataError::MigrationCheckpoint(_)
        | ArtifactDataError::DataContractUnavailable
        | ArtifactDataError::DataContractSchemaInvalid
        | ArtifactDataError::InvalidQuota
        | ArtifactDataError::ObjectIntegrity
        | ArtifactDataError::SnapshotIntegrity
        | ArtifactDataError::Storage(_) => SandboxError::HostCapability {
            capability: capability.clone(),
            message: "artifact data capability is unavailable".to_string(),
        },
    }
}
