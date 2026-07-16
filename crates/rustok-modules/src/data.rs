use async_trait::async_trait;
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value as SqlValue,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use thiserror::Error;
use uuid::Uuid;

use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::OutboxTransport;
use rustok_sandbox::{
    CapabilityBroker, CapabilityCall, CapabilityGrant, CapabilityResponse, SandboxError,
    SandboxResult, SandboxSubject,
};

const MAX_ARTIFACT_DATA_KEY_BYTES: usize = 256;
const MAX_ARTIFACT_DATA_VALUE_BYTES: usize = 64 * 1024;
const MAX_ARTIFACT_DATA_PAGE_SIZE: u32 = 100;

/// Host-owned namespace for untrusted artifact data. Guests never supply a
/// physical table, bucket, database schema, or secret-store location.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataScope {
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub data_contract_revision: u64,
    pub policy_revision: u64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataWrite {
    pub key: String,
    pub value: Value,
    pub expected_revision: Option<u64>,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataRecord {
    pub key: String,
    pub value: Value,
    pub revision: u64,
}

/// A bounded keyset page. The continuation is a validated logical key; guests
/// never receive a database offset or query plan.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPageRequest {
    pub prefix: String,
    pub after_key: Option<String>,
    pub limit: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtifactDataPage {
    pub records: Vec<ArtifactDataRecord>,
    pub next_after_key: Option<String>,
}

/// The operation being authorized by the host. Values are intentionally absent:
/// policy evaluation receives namespace and logical-key context, never an
/// unbounded guest payload.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactDataAccess {
    Read { key: String },
    Write { key: String },
    List,
}

/// An explicit destructive command. The authorizer receives this exact request
/// and is responsible for lifecycle, retention, legal-hold, and actor policy
/// checks before the database transaction begins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPurgeRequest {
    pub scope: ArtifactDataScope,
    pub expected_namespace_revision: u64,
    pub actor_id: Uuid,
    pub reason: String,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDataPurgeResult {
    pub namespace_revision: u64,
    pub purged_records: u64,
}

impl ArtifactDataScope {
    pub fn validate(&self) -> Result<(), ArtifactDataError> {
        if self.tenant_id.is_nil()
            || !valid_module_slug(&self.module_slug)
            || self.data_contract_revision == 0
            || self.policy_revision == 0
        {
            return Err(ArtifactDataError::InvalidScope);
        }
        Ok(())
    }
}

pub fn validate_artifact_data_key(key: &str) -> Result<(), ArtifactDataError> {
    if key.is_empty()
        || key.len() > MAX_ARTIFACT_DATA_KEY_BYTES
        || key.starts_with('/')
        || key.split('/').any(|segment| {
            segment.is_empty() || segment == "." || segment == ".." || segment.contains('\\')
        })
    {
        return Err(ArtifactDataError::InvalidKey);
    }
    Ok(())
}

pub fn validate_artifact_data_prefix(prefix: &str) -> Result<(), ArtifactDataError> {
    let key = prefix
        .strip_suffix('/')
        .filter(|key| !key.ends_with('/'))
        .ok_or(ArtifactDataError::InvalidKey)?;
    validate_artifact_data_key(key)
}

fn validate_artifact_data_value(value: &Value) -> Result<(), ArtifactDataError> {
    let encoded =
        serde_json::to_vec(value).map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
    if encoded.len() > MAX_ARTIFACT_DATA_VALUE_BYTES {
        return Err(ArtifactDataError::ValueTooLarge {
            limit: MAX_ARTIFACT_DATA_VALUE_BYTES,
            actual: encoded.len(),
        });
    }
    Ok(())
}

fn validate_page_request(page: &ArtifactDataPageRequest) -> Result<(), ArtifactDataError> {
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

fn valid_module_slug(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 48
        && !value.starts_with('_')
        && !value.ends_with('_')
        && value.chars().all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '_'
        })
}

#[async_trait]
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

    async fn list(
        &self,
        scope: &ArtifactDataScope,
        page: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataPage, ArtifactDataError>;
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

/// The host owns destructive-data authority. An artifact cannot supply an
/// implementation or replace this check through its broker capability.
#[async_trait]
pub trait ArtifactDataPurgeAuthorizer: Send + Sync {
    async fn authorize_purge(
        &self,
        request: &ArtifactDataPurgeRequest,
    ) -> Result<(), ArtifactDataError>;
}

/// SeaORM adapter for the host-owned structured-value namespace. It never
/// accepts a guest-selected database object, SQL fragment, or storage path.
#[derive(Clone)]
pub struct SeaOrmArtifactDataBroker<A, V> {
    db: DatabaseConnection,
    authorizer: A,
    schema_validator: V,
}

impl<A, V> SeaOrmArtifactDataBroker<A, V>
where
    A: ArtifactDataAuthorizer,
    V: ArtifactDataSchemaValidator,
{
    pub fn new(db: DatabaseConnection, authorizer: A, schema_validator: V) -> Self {
        Self {
            db,
            authorizer,
            schema_validator,
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
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND data_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                scope_values(scope, backend, key)?,
            ))
            .await
            .map_err(storage_error)?;
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
        let backend = transaction.get_database_backend();
        ensure_active_namespace(&transaction, scope, backend).await?;
        if let Some(row) = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT data_key, value, revision, expected_revision FROM module_artifact_data_operations
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    uuid_value(write.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?
        {
            let expected_revision: Option<i64> = row
                .try_get("", "expected_revision")
                .map_err(storage_error)?;
            let record = record_from_row(row)?;
            if record.key != write.key
                || record.value != write.value
                || expected_revision
                    .map(u64::try_from)
                    .transpose()
                    .map_err(|_| ArtifactDataError::IdempotencyConflict)?
                    != write.expected_revision
            {
                return Err(ArtifactDataError::IdempotencyConflict);
            }
            transaction.commit().await.map_err(storage_error)?;
            return Ok(record);
        }

        let current = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND data_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                scope_values(scope, backend, &write.key)?,
            ))
            .await
            .map_err(storage_error)?;
        let revision = if let Some(row) = current {
            let current = record_from_row(row)?;
            if write.expected_revision != Some(current.revision) {
                return Err(ArtifactDataError::RevisionConflict);
            }
            let next_revision = current
                .revision
                .checked_add(1)
                .ok_or(ArtifactDataError::RevisionConflict)?;
            let result = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "UPDATE module_artifact_data SET value = {}, revision = {}, updated_at = {}
                         WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                         AND data_key = {} AND revision = {}",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        now_expression(backend),
                        placeholder(backend, 3),
                        placeholder(backend, 4),
                        placeholder(backend, 5),
                        placeholder(backend, 6),
                        placeholder(backend, 7),
                    ),
                    vec![
                        SqlValue::Json(Some(Box::new(write.value.clone()))),
                        revision_value(next_revision)?,
                        uuid_value(scope.tenant_id, backend),
                        scope.module_slug.clone().into(),
                        revision_value(scope.data_contract_revision)?,
                        write.key.clone().into(),
                        revision_value(current.revision)?,
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(ArtifactDataError::RevisionConflict);
            }
            next_revision
        } else {
            if write.expected_revision.is_some() {
                return Err(ArtifactDataError::RevisionConflict);
            }
            let result = transaction
                .execute(Statement::from_sql_and_values(
                    backend,
                    format!(
                        "INSERT INTO module_artifact_data
                         (tenant_id, module_slug, data_contract_revision, data_key, value, revision, updated_at)
                         VALUES ({}, {}, {}, {}, {}, 1, {}) ON CONFLICT DO NOTHING",
                        placeholder(backend, 1),
                        placeholder(backend, 2),
                        placeholder(backend, 3),
                        placeholder(backend, 4),
                        placeholder(backend, 5),
                        now_expression(backend),
                    ),
                    vec![
                        uuid_value(scope.tenant_id, backend),
                        scope.module_slug.clone().into(),
                        revision_value(scope.data_contract_revision)?,
                        write.key.clone().into(),
                        SqlValue::Json(Some(Box::new(write.value.clone()))),
                    ],
                ))
                .await
                .map_err(storage_error)?;
            if result.rows_affected() != 1 {
                return Err(ArtifactDataError::RevisionConflict);
            }
            1
        };
        let record = ArtifactDataRecord {
            key: write.key,
            value: write.value,
            revision,
        };
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_operations
                     (tenant_id, module_slug, data_contract_revision, idempotency_key, data_key, value, expected_revision, revision, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(scope.tenant_id, backend),
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    uuid_value(write.idempotency_key, backend),
                    record.key.clone().into(),
                    SqlValue::Json(Some(Box::new(record.value.clone()))),
                    optional_revision_value(write.expected_revision)?,
                    revision_value(record.revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(record)
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
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
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
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    prefix_pattern.clone().into(),
                    after_key.into(),
                    query_limit.into(),
                ],
            ),
            None => (
                format!(
                    "SELECT data_key, value, revision FROM module_artifact_data
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
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
                    scope.module_slug.clone().into(),
                    revision_value(scope.data_contract_revision)?,
                    prefix_pattern.into(),
                    query_limit.into(),
                ],
            ),
        };
        let mut records = transaction
            .query_all(Statement::from_sql_and_values(backend, query, values))
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
}

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
    ) -> Self {
        Self {
            data: SeaOrmArtifactDataBroker::new(db, authorizer, schema_validator),
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
        }
    }
}

enum DataCapabilityCall {
    Get { key: String },
    Put { write: ArtifactDataWrite },
    List { page: ArtifactDataPageRequest },
}

fn decode_data_capability_call(call: &CapabilityCall) -> SandboxResult<DataCapabilityCall> {
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
        "put" => {
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
            Ok(DataCapabilityCall::Put {
                write: ArtifactDataWrite {
                    key: required_data_capability_string(call, input, "key")?.to_string(),
                    value,
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
            Ok(DataCapabilityCall::List {
                page: ArtifactDataPageRequest {
                    prefix,
                    after_key,
                    limit: u32::try_from(limit).map_err(|_| {
                        data_capability_constraint(call, "data list limit must fit u32")
                    })?,
                },
            })
        }
        _ => Err(data_capability_constraint(
            call,
            "data operation is unsupported",
        )),
    }
}

fn data_capability_constraint(call: &CapabilityCall, reason: &str) -> SandboxError {
    SandboxError::CapabilityConstraintDenied {
        capability: call.capability.clone(),
        reason: reason.to_string(),
    }
}

fn required_data_capability_string<'a>(
    call: &CapabilityCall,
    input: &'a serde_json::Map<String, Value>,
    field: &str,
) -> SandboxResult<&'a str> {
    input
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| data_capability_constraint(call, &format!("data {field} must be a string")))
}

fn reject_data_capability_fields(
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

fn escape_like_prefix(prefix: &str) -> String {
    prefix
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn data_capability_error(
    capability: &rustok_sandbox::CapabilityName,
    error: ArtifactDataError,
) -> SandboxError {
    match error {
        ArtifactDataError::InvalidScope
        | ArtifactDataError::InvalidKey
        | ArtifactDataError::InvalidPage
        | ArtifactDataError::RevisionConflict
        | ArtifactDataError::NamespacePurged
        | ArtifactDataError::PurgePrecondition
        | ArtifactDataError::InvalidIdempotencyKey
        | ArtifactDataError::IdempotencyConflict
        | ArtifactDataError::ValueTooLarge { .. }
        | ArtifactDataError::PolicyDenied => SandboxError::CapabilityDenied(capability.clone()),
        ArtifactDataError::Storage(_) => SandboxError::HostCapability {
            capability: capability.clone(),
            message: "artifact data capability is unavailable".to_string(),
        },
    }
}

/// Owner service for irreversible namespace deletion. Its authorization port
/// keeps retention and installation lifecycle policy outside guest-controlled
/// capability calls while the data owner keeps mutation, audit and outbox facts
/// in one transaction.
#[derive(Clone)]
pub struct SeaOrmArtifactDataPurgeService<A> {
    db: DatabaseConnection,
    authorizer: A,
}

impl<A> SeaOrmArtifactDataPurgeService<A>
where
    A: ArtifactDataPurgeAuthorizer,
{
    pub fn new(db: DatabaseConnection, authorizer: A) -> Self {
        Self { db, authorizer }
    }

    pub async fn purge(
        &self,
        request: ArtifactDataPurgeRequest,
    ) -> Result<ArtifactDataPurgeResult, ArtifactDataError> {
        validate_purge_request(&request)?;
        self.authorizer.authorize_purge(&request).await?;
        let transaction = self.db.begin().await.map_err(storage_error)?;
        configure_tenant_scope(&transaction, request.scope.tenant_id).await?;
        let backend = transaction.get_database_backend();
        if let Some(row) = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT expected_namespace_revision, actor_id, reason, namespace_revision, purged_records
                     FROM module_artifact_data_purge_operations
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {} AND idempotency_key = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)?,
                    uuid_value(request.idempotency_key, backend),
                ],
            ))
            .await
            .map_err(storage_error)?
        {
            let expected_revision: i64 = row
                .try_get("", "expected_namespace_revision")
                .map_err(storage_error)?;
            let actor_id = uuid_from_row(&row, "actor_id", backend)?;
            let reason: String = row.try_get("", "reason").map_err(storage_error)?;
            if u64::try_from(expected_revision).ok() != Some(request.expected_namespace_revision)
                || actor_id != request.actor_id
                || reason != request.reason
            {
                return Err(ArtifactDataError::IdempotencyConflict);
            }
            let namespace_revision: i64 = row
                .try_get("", "namespace_revision")
                .map_err(storage_error)?;
            let purged_records: i64 = row
                .try_get("", "purged_records")
                .map_err(storage_error)?;
            transaction.commit().await.map_err(storage_error)?;
            return Ok(ArtifactDataPurgeResult {
                namespace_revision: u64::try_from(namespace_revision)
                    .map_err(|_| ArtifactDataError::PurgePrecondition)?,
                purged_records: u64::try_from(purged_records)
                    .map_err(|_| ArtifactDataError::PurgePrecondition)?,
            });
        }

        let namespace = transaction
            .query_one(Statement::from_sql_and_values(
                backend,
                format!(
                    "SELECT namespace_revision, CASE WHEN purged_at IS NULL THEN 0 ELSE 1 END AS is_purged
                     FROM module_artifact_data_namespaces
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}{}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    namespace_lock_clause(backend),
                ),
                namespace_values(&request.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .ok_or(ArtifactDataError::PurgePrecondition)?;
        let current_revision: i64 = namespace
            .try_get("", "namespace_revision")
            .map_err(storage_error)?;
        let already_purged = namespace
            .try_get::<i64>("", "is_purged")
            .map_err(storage_error)?
            != 0;
        if already_purged
            || u64::try_from(current_revision).ok() != Some(request.expected_namespace_revision)
        {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        let purged_records = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(&request.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?
            .rows_affected();
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "DELETE FROM module_artifact_data_operations
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                ),
                namespace_values(&request.scope, backend)?,
            ))
            .await
            .map_err(storage_error)?;
        let next_revision = u64::try_from(current_revision)
            .ok()
            .and_then(|value| value.checked_add(1))
            .ok_or(ArtifactDataError::PurgePrecondition)?;
        let updated = transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_data_namespaces
                     SET namespace_revision = {}, purged_at = {}, updated_at = {}
                     WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                     AND namespace_revision = {} AND purged_at IS NULL",
                    placeholder(backend, 1),
                    now_expression(backend),
                    now_expression(backend),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                ),
                vec![
                    revision_value(next_revision)?,
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)?,
                    revision_value(request.expected_namespace_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if updated.rows_affected() != 1 {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        let purged_records =
            i64::try_from(purged_records).map_err(|_| ArtifactDataError::PurgePrecondition)?;
        transaction
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "INSERT INTO module_artifact_data_purge_operations
                     (tenant_id, module_slug, data_contract_revision, idempotency_key, expected_namespace_revision,
                      namespace_revision, actor_id, reason, purged_records, completed_at)
                     VALUES ({}, {}, {}, {}, {}, {}, {}, {}, {}, {})",
                    placeholder(backend, 1),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                    placeholder(backend, 5),
                    placeholder(backend, 6),
                    placeholder(backend, 7),
                    placeholder(backend, 8),
                    placeholder(backend, 9),
                    now_expression(backend),
                ),
                vec![
                    uuid_value(request.scope.tenant_id, backend),
                    request.scope.module_slug.clone().into(),
                    revision_value(request.scope.data_contract_revision)?,
                    uuid_value(request.idempotency_key, backend),
                    revision_value(request.expected_namespace_revision)?,
                    revision_value(next_revision)?,
                    uuid_value(request.actor_id, backend),
                    request.reason.clone().into(),
                    purged_records.into(),
                ],
            ))
            .await
            .map_err(storage_error)?;
        OutboxTransport::new(self.db.clone())
            .write_to_outbox(
                &transaction,
                EventEnvelope::new(
                    Uuid::new_v4(),
                    Some(request.scope.tenant_id),
                    DomainEvent::ModuleArtifactDataPurged {
                        tenant_id: request.scope.tenant_id,
                        module_slug: request.scope.module_slug.clone(),
                        data_contract_revision: request.scope.data_contract_revision,
                        namespace_revision: next_revision,
                        purged_records: u64::try_from(purged_records)
                            .map_err(|_| ArtifactDataError::PurgePrecondition)?,
                    },
                ),
            )
            .await
            .map_err(storage_error)?;
        transaction.commit().await.map_err(storage_error)?;
        Ok(ArtifactDataPurgeResult {
            namespace_revision: next_revision,
            purged_records: u64::try_from(purged_records)
                .map_err(|_| ArtifactDataError::PurgePrecondition)?,
        })
    }
}

pub(crate) async fn configure_tenant_scope<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
) -> Result<(), ArtifactDataError> {
    if connection.get_database_backend() == DbBackend::Postgres {
        connection
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('rustok.tenant_id', $1, true)",
                vec![tenant_id.to_string().into()],
            ))
            .await
            .map_err(storage_error)?;
    }
    Ok(())
}

async fn ensure_active_namespace<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
    backend: DbBackend,
) -> Result<(), ArtifactDataError> {
    connection
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_namespaces
                 (tenant_id, module_slug, data_contract_revision, namespace_revision, created_at, updated_at)
                 VALUES ({}, {}, {}, 1, {}, {}) ON CONFLICT DO NOTHING",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                now_expression(backend),
                now_expression(backend),
            ),
            namespace_values(scope, backend)?,
        ))
        .await
        .map_err(storage_error)?;
    let active = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT namespace_revision FROM module_artifact_data_namespaces
                 WHERE tenant_id = {} AND module_slug = {} AND data_contract_revision = {}
                 AND purged_at IS NULL{}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                namespace_lock_clause(backend),
            ),
            namespace_values(scope, backend)?,
        ))
        .await
        .map_err(storage_error)?;
    if active.is_none() {
        return Err(ArtifactDataError::NamespacePurged);
    }
    Ok(())
}

fn validate_purge_request(request: &ArtifactDataPurgeRequest) -> Result<(), ArtifactDataError> {
    request.scope.validate()?;
    if request.expected_namespace_revision == 0
        || request.actor_id.is_nil()
        || request.idempotency_key.is_nil()
        || request.reason.trim().is_empty()
        || request.reason.len() > 2_000
    {
        return Err(ArtifactDataError::PurgePrecondition);
    }
    Ok(())
}

fn scope_values(
    scope: &ArtifactDataScope,
    backend: DbBackend,
    key: &str,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        uuid_value(scope.tenant_id, backend),
        scope.module_slug.clone().into(),
        revision_value(scope.data_contract_revision)?,
        key.to_owned().into(),
    ])
}

fn namespace_values(
    scope: &ArtifactDataScope,
    backend: DbBackend,
) -> Result<Vec<SqlValue>, ArtifactDataError> {
    Ok(vec![
        uuid_value(scope.tenant_id, backend),
        scope.module_slug.clone().into(),
        revision_value(scope.data_contract_revision)?,
    ])
}

pub(crate) fn revision_value(value: u64) -> Result<SqlValue, ArtifactDataError> {
    i64::try_from(value)
        .map(|value| value.into())
        .map_err(|_| ArtifactDataError::RevisionConflict)
}

pub(crate) fn optional_revision_value(value: Option<u64>) -> Result<SqlValue, ArtifactDataError> {
    value
        .map(revision_value)
        .transpose()
        .map(|value| match value {
            Some(value) => value,
            None => SqlValue::BigInt(None),
        })
}

pub(crate) fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::Uuid(Some(Box::new(value))),
        _ => value.to_string().into(),
    }
}

pub(crate) fn uuid_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ArtifactDataError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        _ => row
            .try_get::<String>("", column)
            .map_err(storage_error)?
            .parse()
            .map_err(storage_error),
    }
}

pub(crate) fn placeholder(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Postgres => format!("${index}"),
        _ => format!("?{index}"),
    }
}

pub(crate) fn now_expression(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "NOW()",
        _ => "datetime('now')",
    }
}

pub(crate) fn namespace_lock_clause(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => " FOR UPDATE",
        _ => "",
    }
}

fn record_from_row(row: sea_orm::QueryResult) -> Result<ArtifactDataRecord, ArtifactDataError> {
    let revision: i64 = row.try_get("", "revision").map_err(storage_error)?;
    Ok(ArtifactDataRecord {
        key: row.try_get("", "data_key").map_err(storage_error)?,
        value: row.try_get("", "value").map_err(storage_error)?,
        revision: u64::try_from(revision).map_err(|_| ArtifactDataError::RevisionConflict)?,
    })
}

fn storage_error(error: impl std::fmt::Display) -> ArtifactDataError {
    ArtifactDataError::Storage(error.to_string())
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ArtifactDataError {
    #[error("artifact data scope is invalid")]
    InvalidScope,
    #[error("artifact data key is invalid")]
    InvalidKey,
    #[error("artifact data page is invalid")]
    InvalidPage,
    #[error("artifact data revision conflict")]
    RevisionConflict,
    #[error("artifact data namespace was purged")]
    NamespacePurged,
    #[error("artifact data purge precondition failed")]
    PurgePrecondition,
    #[error("artifact data idempotency key is invalid")]
    InvalidIdempotencyKey,
    #[error("artifact data idempotency key was reused for a different key")]
    IdempotencyConflict,
    #[error("artifact data value exceeds {limit} bytes (received {actual})")]
    ValueTooLarge { limit: usize, actual: usize },
    #[error("artifact data policy denied the operation")]
    PolicyDenied,
    #[error("artifact data storage failed: {0}")]
    Storage(String),
}

#[cfg(test)]
mod tests {
    use rustok_sandbox::{
        CapabilityCall, CapabilityCallContext, CapabilityName, ExecutionPhase, SandboxSubject,
    };
    use serde_json::json;

    use super::*;

    #[test]
    fn scope_and_keys_reject_guest_controlled_namespace_escapes() {
        assert!(matches!(
            ArtifactDataScope {
                tenant_id: Uuid::nil(),
                module_slug: "module".into(),
                data_contract_revision: 1,
                policy_revision: 1,
            }
            .validate(),
            Err(ArtifactDataError::InvalidScope)
        ));
        for key in ["/host/path", "state/../escape", "state//key"] {
            assert!(matches!(
                validate_artifact_data_key(key),
                Err(ArtifactDataError::InvalidKey)
            ));
        }
    }

    #[test]
    fn sandbox_data_adapter_keeps_list_continuations_inside_the_prefix() {
        let mut call = CapabilityCall {
            execution_id: Uuid::new_v4(),
            subject: SandboxSubject::ModuleArtifact {
                slug: "sample_module".to_string(),
                version: "1.0.0".to_string(),
                digest: "sha256:sample".to_string(),
            },
            context: CapabilityCallContext {
                phase: ExecutionPhase::Lifecycle,
                tenant_id: Some(Uuid::new_v4()),
                actor_id: None,
                trace_id: None,
            },
            capability: CapabilityName::new("platform.data").expect("capability name"),
            operation: "list".to_string(),
            input: json!({ "prefix": "state/", "after_key": "state/one", "limit": 10 }),
        };
        assert!(matches!(
            decode_data_capability_call(&call),
            Ok(DataCapabilityCall::List { .. })
        ));
        call.input = json!({ "prefix": "state/", "after_key": "other/one", "limit": 10 });
        assert!(decode_data_capability_call(&call).is_err());
    }
}
