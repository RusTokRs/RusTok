use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, QueryResult, Statement,
    TransactionTrait, Value as SqlValue,
};
use serde_json::{Value as JsonValue, json};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;

use super::drift_finding_inspector::{IndexDriftFindingScope, IndexDriftFindingSeverity};

const DIGEST_BYTES: usize = 64;
const MAX_CHECK_NAME_BYTES: usize = 128;
const MAX_LOCALE_BYTES: usize = 32;
const FINDING_KEY_CONTRACT: &[u8] = b"index_drift_finding_key_v1";
const NO_LOCALE_KEY_COMPONENT: &[u8] = b"\0";
const FINDING_DETAILS_CONTRACT: &str = "index_drift_digest_finding_v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftDigestFindingRequest {
    tenant_id: Uuid,
    check_name: String,
    severity: IndexDriftFindingSeverity,
    scope: IndexDriftFindingScope,
    expected_digest: String,
    actual_digest: String,
    finding_key: String,
}

impl IndexDriftDigestFindingRequest {
    pub fn new(
        tenant_id: Uuid,
        check_name: impl Into<String>,
        severity: IndexDriftFindingSeverity,
        scope: IndexDriftFindingScope,
        expected_digest: impl Into<String>,
        actual_digest: impl Into<String>,
    ) -> Result<Self, IndexDriftFindingWriteError> {
        if tenant_id.is_nil() {
            return Err(IndexDriftFindingWriteError::NilTenantId);
        }
        let check_name = check_name.into();
        validate_check_name(&check_name)?;
        PersistedFindingScope::from_scope(&scope)?;
        let expected_digest = expected_digest.into();
        let actual_digest = actual_digest.into();
        validate_digest(&expected_digest, "expected digest")?;
        validate_digest(&actual_digest, "actual digest")?;
        if expected_digest == actual_digest {
            return Err(IndexDriftFindingWriteError::EqualDigests);
        }
        let finding_key = derive_finding_key(tenant_id, &check_name, &scope);
        Ok(Self {
            tenant_id,
            check_name,
            severity,
            scope,
            expected_digest,
            actual_digest,
            finding_key,
        })
    }

    pub fn tenant_id(&self) -> Uuid {
        self.tenant_id
    }

    pub fn check_name(&self) -> &str {
        &self.check_name
    }

    pub fn severity(&self) -> IndexDriftFindingSeverity {
        self.severity
    }

    pub fn scope(&self) -> &IndexDriftFindingScope {
        &self.scope
    }

    pub fn expected_digest(&self) -> &str {
        &self.expected_digest
    }

    pub fn actual_digest(&self) -> &str {
        &self.actual_digest
    }

    pub fn finding_key(&self) -> &str {
        &self.finding_key
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftFindingWriteOutcome {
    Created {
        finding_id: Uuid,
        finding_key: String,
    },
    Refreshed {
        finding_id: Uuid,
        finding_key: String,
    },
    Reopened {
        finding_id: Uuid,
        finding_key: String,
    },
    Suppressed {
        finding_id: Uuid,
        finding_key: String,
    },
}

impl IndexDriftFindingWriteOutcome {
    pub fn finding_id(&self) -> Uuid {
        match self {
            Self::Created { finding_id, .. }
            | Self::Refreshed { finding_id, .. }
            | Self::Reopened { finding_id, .. }
            | Self::Suppressed { finding_id, .. } => *finding_id,
        }
    }

    pub fn finding_key(&self) -> &str {
        match self {
            Self::Created { finding_key, .. }
            | Self::Refreshed { finding_key, .. }
            | Self::Reopened { finding_key, .. }
            | Self::Suppressed { finding_key, .. } => finding_key,
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IndexDriftFindingWriteError {
    #[error("Index drift finding writer tenant id must not be nil")]
    NilTenantId,
    #[error("Index drift finding check name is invalid")]
    InvalidCheckName,
    #[error("Index drift finding scope is invalid: {0}")]
    InvalidScope(&'static str),
    #[error("Index drift finding {0} is outside the lowercase SHA-256 contract")]
    InvalidDigest(&'static str),
    #[error("Index drift finding expected and actual digests must differ")]
    EqualDigests,
    #[error("stored Index drift finding conflicts with the deterministic finding key contract")]
    StoredContractConflict,
    #[error("stored Index drift finding state is unsupported")]
    InvalidStoredState,
    #[error("Index drift finding writer does not support this database backend")]
    UnsupportedBackend,
    #[error("Index drift finding writer storage operation failed")]
    Storage,
}

/// Index-owned persistence boundary for one exact source/index digest mismatch.
///
/// The writer serializes one tenant/finding key, preserves finding identity and
/// first-detected time, refreshes bounded digest evidence for open findings,
/// reopens resolved findings, and keeps ignored findings suppressed. The caller
/// cannot provide raw JSON details, timestamps, finding UUIDs, or lifecycle state.
#[derive(Clone)]
pub struct PostgresIndexDriftFindingWriter {
    db: DatabaseConnection,
}

impl PostgresIndexDriftFindingWriter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn record_digest_mismatch(
        &self,
        request: &IndexDriftDigestFindingRequest,
    ) -> Result<IndexDriftFindingWriteOutcome, IndexDriftFindingWriteError> {
        let transaction = self.db.begin().await.map_err(storage_error)?;
        let result = self.record_in_transaction(&transaction, request).await;
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

    async fn record_in_transaction(
        &self,
        transaction: &DatabaseTransaction,
        request: &IndexDriftDigestFindingRequest,
    ) -> Result<IndexDriftFindingWriteOutcome, IndexDriftFindingWriteError> {
        let backend = transaction.get_database_backend();
        ensure_supported_backend(backend)?;
        lock_finding_key(transaction, request, backend).await?;
        let expected_scope = PersistedFindingScope::from_scope(request.scope())?;

        if let Some(existing) = load_existing_finding(transaction, request, backend).await? {
            return refresh_existing_finding(
                transaction,
                request,
                &expected_scope,
                existing,
                backend,
            )
            .await;
        }

        let finding_id = Uuid::new_v4();
        let inserted = transaction
            .execute_raw(Statement::from_sql_and_values(
                backend,
                insert_finding_sql(backend),
                insert_values(request, finding_id, &expected_scope, backend),
            ))
            .await
            .map_err(storage_error)?;
        if inserted.rows_affected() == 1 {
            return Ok(IndexDriftFindingWriteOutcome::Created {
                finding_id,
                finding_key: request.finding_key().to_owned(),
            });
        }

        let existing = load_existing_finding(transaction, request, backend)
            .await?
            .ok_or(IndexDriftFindingWriteError::Storage)?;
        refresh_existing_finding(transaction, request, &expected_scope, existing, backend).await
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PersistedFindingScope {
    scope_kind: String,
    module_name: Option<String>,
    entity_name: Option<String>,
    schema_version: Option<i64>,
    entity_id: Option<Uuid>,
    locale_key: Option<String>,
}

impl PersistedFindingScope {
    fn from_scope(scope: &IndexDriftFindingScope) -> Result<Self, IndexDriftFindingWriteError> {
        match scope {
            IndexDriftFindingScope::Global => Ok(Self {
                scope_kind: "global".to_owned(),
                module_name: None,
                entity_name: None,
                schema_version: None,
                entity_id: None,
                locale_key: None,
            }),
            IndexDriftFindingScope::Schema { schema } => {
                let version = validate_schema_version(schema.version.get())?;
                Ok(Self {
                    scope_kind: "schema".to_owned(),
                    module_name: Some(schema.module.as_str().to_owned()),
                    entity_name: Some(schema.entity.as_str().to_owned()),
                    schema_version: Some(version),
                    entity_id: None,
                    locale_key: None,
                })
            }
            IndexDriftFindingScope::Entity {
                schema,
                entity_id,
                locale,
            } => {
                if entity_id.is_nil() {
                    return Err(IndexDriftFindingWriteError::InvalidScope(
                        "entity id must not be nil",
                    ));
                }
                if locale.as_str().len() > MAX_LOCALE_BYTES {
                    return Err(IndexDriftFindingWriteError::InvalidScope(
                        "locale exceeds the persisted scope limit",
                    ));
                }
                let version = validate_schema_version(schema.version.get())?;
                Ok(Self {
                    scope_kind: "entity".to_owned(),
                    module_name: Some(schema.module.as_str().to_owned()),
                    entity_name: Some(schema.entity.as_str().to_owned()),
                    schema_version: Some(version),
                    entity_id: Some(*entity_id),
                    locale_key: Some(locale.as_str().to_owned()),
                })
            }
            IndexDriftFindingScope::EntityWithoutLocale { schema, entity_id } => {
                if entity_id.is_nil() {
                    return Err(IndexDriftFindingWriteError::InvalidScope(
                        "entity id must not be nil",
                    ));
                }
                let version = validate_schema_version(schema.version.get())?;
                Ok(Self {
                    scope_kind: "entity".to_owned(),
                    module_name: Some(schema.module.as_str().to_owned()),
                    entity_name: Some(schema.entity.as_str().to_owned()),
                    schema_version: Some(version),
                    entity_id: Some(*entity_id),
                    locale_key: None,
                })
            }
        }
    }
}

#[derive(Debug)]
struct StoredFinding {
    finding_id: Uuid,
    check_name: String,
    state: String,
    scope: PersistedFindingScope,
}

async fn refresh_existing_finding(
    transaction: &DatabaseTransaction,
    request: &IndexDriftDigestFindingRequest,
    expected_scope: &PersistedFindingScope,
    existing: StoredFinding,
    backend: DbBackend,
) -> Result<IndexDriftFindingWriteOutcome, IndexDriftFindingWriteError> {
    if existing.check_name != request.check_name() || existing.scope != *expected_scope {
        return Err(IndexDriftFindingWriteError::StoredContractConflict);
    }

    let (sql, outcome) = match existing.state.as_str() {
        "open" => (
            refresh_open_finding_sql(backend),
            IndexDriftFindingWriteOutcome::Refreshed {
                finding_id: existing.finding_id,
                finding_key: request.finding_key().to_owned(),
            },
        ),
        "resolved" => (
            reopen_resolved_finding_sql(backend),
            IndexDriftFindingWriteOutcome::Reopened {
                finding_id: existing.finding_id,
                finding_key: request.finding_key().to_owned(),
            },
        ),
        "ignored" => (
            refresh_ignored_finding_sql(backend),
            IndexDriftFindingWriteOutcome::Suppressed {
                finding_id: existing.finding_id,
                finding_key: request.finding_key().to_owned(),
            },
        ),
        _ => return Err(IndexDriftFindingWriteError::InvalidStoredState),
    };

    let updated = transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            sql,
            update_values(request, existing.finding_id, backend),
        ))
        .await
        .map_err(storage_error)?;
    if updated.rows_affected() != 1 {
        return Err(IndexDriftFindingWriteError::Storage);
    }
    Ok(outcome)
}

async fn lock_finding_key(
    transaction: &DatabaseTransaction,
    request: &IndexDriftDigestFindingRequest,
    backend: DbBackend,
) -> Result<(), IndexDriftFindingWriteError> {
    if backend == DbBackend::Sqlite {
        return Ok(());
    }
    let lock_key = format!(
        "index-drift-finding\u{1f}{}\u{1f}{}",
        request.tenant_id(),
        request.finding_key(),
    );
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))",
            vec![lock_key.into()],
        ))
        .await
        .map_err(storage_error)?;
    Ok(())
}

async fn load_existing_finding(
    transaction: &DatabaseTransaction,
    request: &IndexDriftDigestFindingRequest,
    backend: DbBackend,
) -> Result<Option<StoredFinding>, IndexDriftFindingWriteError> {
    transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            select_existing_finding_sql(backend),
            vec![
                uuid_value(request.tenant_id(), backend),
                request.finding_key().to_owned().into(),
            ],
        ))
        .await
        .map_err(storage_error)?
        .map(|row| stored_finding(&row, backend))
        .transpose()
}

fn stored_finding(
    row: &QueryResult,
    backend: DbBackend,
) -> Result<StoredFinding, IndexDriftFindingWriteError> {
    Ok(StoredFinding {
        finding_id: required_uuid(row, "finding_id", backend)?,
        check_name: row.try_get("", "check_name").map_err(storage_error)?,
        state: row.try_get("", "state").map_err(storage_error)?,
        scope: PersistedFindingScope {
            scope_kind: row.try_get("", "scope_kind").map_err(storage_error)?,
            module_name: row.try_get("", "module_name").map_err(storage_error)?,
            entity_name: row.try_get("", "entity_name").map_err(storage_error)?,
            schema_version: row
                .try_get("", "schema_version_value")
                .map_err(storage_error)?,
            entity_id: optional_uuid(row, "entity_id", backend)?,
            locale_key: row.try_get("", "locale_key").map_err(storage_error)?,
        },
    })
}

fn finding_details() -> JsonValue {
    json!({ "contract": FINDING_DETAILS_CONTRACT })
}

fn insert_values(
    request: &IndexDriftDigestFindingRequest,
    finding_id: Uuid,
    scope: &PersistedFindingScope,
    backend: DbBackend,
) -> Vec<SqlValue> {
    vec![
        uuid_value(request.tenant_id(), backend),
        uuid_value(finding_id, backend),
        request.finding_key().to_owned().into(),
        request.check_name().to_owned().into(),
        severity_value(request.severity()).to_owned().into(),
        scope.scope_kind.clone().into(),
        scope.module_name.clone().into(),
        scope.entity_name.clone().into(),
        scope.schema_version.into(),
        optional_uuid_value(scope.entity_id, backend),
        scope.locale_key.clone().into(),
        request.expected_digest().to_owned().into(),
        request.actual_digest().to_owned().into(),
        SqlValue::Json(Some(Box::new(finding_details()))),
    ]
}

fn update_values(
    request: &IndexDriftDigestFindingRequest,
    finding_id: Uuid,
    backend: DbBackend,
) -> Vec<SqlValue> {
    vec![
        uuid_value(request.tenant_id(), backend),
        uuid_value(finding_id, backend),
        severity_value(request.severity()).to_owned().into(),
        request.expected_digest().to_owned().into(),
        request.actual_digest().to_owned().into(),
        SqlValue::Json(Some(Box::new(finding_details()))),
        request.finding_key().to_owned().into(),
    ]
}

fn derive_finding_key(tenant_id: Uuid, check_name: &str, scope: &IndexDriftFindingScope) -> String {
    let mut hasher = Sha256::new();
    hash_component(&mut hasher, FINDING_KEY_CONTRACT);
    hash_component(&mut hasher, tenant_id.as_bytes());
    hash_component(&mut hasher, check_name.as_bytes());
    match scope {
        IndexDriftFindingScope::Global => hash_component(&mut hasher, b"global"),
        IndexDriftFindingScope::Schema { schema } => {
            hash_component(&mut hasher, b"schema");
            hash_schema(&mut hasher, schema);
        }
        IndexDriftFindingScope::Entity {
            schema,
            entity_id,
            locale,
        } => {
            hash_component(&mut hasher, b"entity");
            hash_schema(&mut hasher, schema);
            hash_component(&mut hasher, entity_id.as_bytes());
            hash_component(&mut hasher, locale.as_str().as_bytes());
        }
        IndexDriftFindingScope::EntityWithoutLocale { schema, entity_id } => {
            hash_component(&mut hasher, b"entity");
            hash_schema(&mut hasher, schema);
            hash_component(&mut hasher, entity_id.as_bytes());
            hash_component(&mut hasher, NO_LOCALE_KEY_COMPONENT);
        }
    }
    hex::encode(hasher.finalize())
}

fn hash_schema(hasher: &mut Sha256, schema: &crate::SchemaRef) {
    hash_component(hasher, schema.module.as_str().as_bytes());
    hash_component(hasher, schema.entity.as_str().as_bytes());
    hash_component(hasher, &schema.version.get().to_be_bytes());
}

fn hash_component(hasher: &mut Sha256, value: &[u8]) {
    let length = u64::try_from(value.len()).expect("bounded finding-key component length");
    hasher.update(length.to_be_bytes());
    hasher.update(value);
}

fn validate_check_name(value: &str) -> Result<(), IndexDriftFindingWriteError> {
    if value.is_empty()
        || value.len() > MAX_CHECK_NAME_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(IndexDriftFindingWriteError::InvalidCheckName);
    }
    Ok(())
}

fn validate_schema_version(value: u32) -> Result<i64, IndexDriftFindingWriteError> {
    if value == 0 {
        return Err(IndexDriftFindingWriteError::InvalidScope(
            "schema version must be positive",
        ));
    }
    Ok(i64::from(value))
}

fn validate_digest(value: &str, label: &'static str) -> Result<(), IndexDriftFindingWriteError> {
    if value.len() != DIGEST_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(IndexDriftFindingWriteError::InvalidDigest(label));
    }
    Ok(())
}

fn severity_value(value: IndexDriftFindingSeverity) -> &'static str {
    match value {
        IndexDriftFindingSeverity::Info => "info",
        IndexDriftFindingSeverity::Warning => "warning",
        IndexDriftFindingSeverity::Error => "error",
    }
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexDriftFindingWriteError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(IndexDriftFindingWriteError::UnsupportedBackend),
    }
}

fn storage_error(_error: impl std::fmt::Display) -> IndexDriftFindingWriteError {
    IndexDriftFindingWriteError::Storage
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn optional_uuid_value(value: Option<Uuid>, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.map(|value| value.to_string()).into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn required_uuid(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, IndexDriftFindingWriteError> {
    let value = match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error)?,
        DbBackend::Sqlite => {
            let value: String = row.try_get("", column).map_err(storage_error)?;
            Uuid::parse_str(&value).map_err(storage_error)?
        }
        _ => return Err(IndexDriftFindingWriteError::UnsupportedBackend),
    };
    if value.is_nil() {
        return Err(IndexDriftFindingWriteError::StoredContractConflict);
    }
    Ok(value)
}

fn optional_uuid(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<Uuid>, IndexDriftFindingWriteError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(storage_error),
        DbBackend::Sqlite => {
            let value: Option<String> = row.try_get("", column).map_err(storage_error)?;
            value
                .map(|value| Uuid::parse_str(&value).map_err(storage_error))
                .transpose()
        }
        _ => Err(IndexDriftFindingWriteError::UnsupportedBackend),
    }
}

fn insert_finding_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "INSERT INTO index_consistency_findings (tenant_id, finding_id, finding_key, check_name, severity, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, expected_digest, actual_digest, details) VALUES ($1, $2, $3, $4, $5, 'open', $6, $7, $8, $9, $10, $11, $12, $13, $14) ON CONFLICT (tenant_id, finding_key) DO NOTHING"
        }
        DbBackend::Sqlite => {
            "INSERT INTO index_consistency_findings (tenant_id, finding_id, finding_key, check_name, severity, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, expected_digest, actual_digest, details) VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14) ON CONFLICT (tenant_id, finding_key) DO NOTHING"
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn select_existing_finding_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "SELECT finding_id, check_name, state, scope_kind, module_name, entity_name, CAST(schema_version AS BIGINT) AS schema_version_value, entity_id, locale_key FROM index_consistency_findings WHERE tenant_id = $1 AND finding_key = $2 FOR UPDATE"
        }
        DbBackend::Sqlite => {
            "SELECT finding_id, check_name, state, scope_kind, module_name, entity_name, CAST(schema_version AS INTEGER) AS schema_version_value, entity_id, locale_key FROM index_consistency_findings WHERE tenant_id = ?1 AND finding_key = ?2"
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn refresh_open_finding_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "UPDATE index_consistency_findings SET severity = $3, expected_digest = $4, actual_digest = $5, details = $6, last_detected_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND finding_id = $2 AND finding_key = $7 AND state = 'open'"
        }
        DbBackend::Sqlite => {
            "UPDATE index_consistency_findings SET severity = ?3, expected_digest = ?4, actual_digest = ?5, details = ?6, last_detected_at = CURRENT_TIMESTAMP WHERE tenant_id = ?1 AND finding_id = ?2 AND finding_key = ?7 AND state = 'open'"
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn reopen_resolved_finding_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "UPDATE index_consistency_findings SET severity = $3, state = 'open', expected_digest = $4, actual_digest = $5, details = $6, last_detected_at = CURRENT_TIMESTAMP, closed_at = NULL WHERE tenant_id = $1 AND finding_id = $2 AND finding_key = $7 AND state = 'resolved'"
        }
        DbBackend::Sqlite => {
            "UPDATE index_consistency_findings SET severity = ?3, state = 'open', expected_digest = ?4, actual_digest = ?5, details = ?6, last_detected_at = CURRENT_TIMESTAMP, closed_at = NULL WHERE tenant_id = ?1 AND finding_id = ?2 AND finding_key = ?7 AND state = 'resolved'"
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn refresh_ignored_finding_sql(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => {
            "UPDATE index_consistency_findings SET severity = $3, expected_digest = $4, actual_digest = $5, details = $6, last_detected_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND finding_id = $2 AND finding_key = $7 AND state = 'ignored'"
        }
        DbBackend::Sqlite => {
            "UPDATE index_consistency_findings SET severity = ?3, expected_digest = ?4, actual_digest = ?5, details = ?6, last_detected_at = CURRENT_TIMESTAMP WHERE tenant_id = ?1 AND finding_id = ?2 AND finding_key = ?7 AND state = 'ignored'"
        }
        _ => unreachable!("unsupported database backend was validated"),
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{Database, DatabaseConnection};

    use super::*;
    use crate::{EntityName, LocaleKey, ModuleName, SchemaRef, SchemaVersion};

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        db.execute_unprepared(
            "CREATE TABLE index_consistency_findings (tenant_id TEXT NOT NULL, finding_id TEXT NOT NULL, finding_key TEXT NOT NULL, check_name TEXT NOT NULL, severity TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'open', scope_kind TEXT NOT NULL, module_name TEXT, entity_name TEXT, schema_version INTEGER, entity_id TEXT, locale_key TEXT, expected_digest TEXT, actual_digest TEXT, details TEXT NOT NULL, first_detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, last_detected_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP, closed_at TEXT, PRIMARY KEY (tenant_id, finding_id), UNIQUE (tenant_id, finding_key))",
        )
        .await
        .expect("finding table");
        db
    }

    fn scope() -> IndexDriftFindingScope {
        IndexDriftFindingScope::Entity {
            schema: SchemaRef {
                module: ModuleName::new("rustok-product").unwrap(),
                entity: EntityName::new("product").unwrap(),
                version: SchemaVersion::new(2),
            },
            entity_id: Uuid::new_v4(),
            locale: LocaleKey::new("en-US").unwrap(),
        }
    }

    fn request(
        tenant_id: Uuid,
        scope: IndexDriftFindingScope,
        actual: char,
    ) -> IndexDriftDigestFindingRequest {
        IndexDriftDigestFindingRequest::new(
            tenant_id,
            "source_index_digest_mismatch",
            IndexDriftFindingSeverity::Error,
            scope,
            "a".repeat(DIGEST_BYTES),
            actual.to_string().repeat(DIGEST_BYTES),
        )
        .unwrap()
    }

    #[test]
    fn request_is_bounded_and_key_is_scope_stable() {
        let tenant_id = Uuid::new_v4();
        let scope = scope();
        let first = request(tenant_id, scope.clone(), 'b');
        let refreshed = request(tenant_id, scope.clone(), 'c');
        assert_eq!(first.finding_key(), refreshed.finding_key());
        assert_ne!(
            first.finding_key(),
            request(Uuid::new_v4(), scope, 'b').finding_key()
        );
        assert!(
            IndexDriftDigestFindingRequest::new(
                tenant_id,
                "source_index_digest_mismatch",
                IndexDriftFindingSeverity::Error,
                IndexDriftFindingScope::Global,
                "a".repeat(DIGEST_BYTES),
                "a".repeat(DIGEST_BYTES),
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn writer_creates_refreshes_reopens_and_preserves_ignored_suppression() {
        let db = database().await;
        let writer = PostgresIndexDriftFindingWriter::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let scope = scope();
        let first = writer
            .record_digest_mismatch(&request(tenant_id, scope.clone(), 'b'))
            .await
            .unwrap();
        assert!(matches!(
            first,
            IndexDriftFindingWriteOutcome::Created { .. }
        ));
        let finding_id = first.finding_id();

        let refreshed = writer
            .record_digest_mismatch(&request(tenant_id, scope.clone(), 'c'))
            .await
            .unwrap();
        assert!(matches!(
            refreshed,
            IndexDriftFindingWriteOutcome::Refreshed { .. }
        ));
        assert_eq!(refreshed.finding_id(), finding_id);

        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE index_consistency_findings SET state = 'resolved', closed_at = CURRENT_TIMESTAMP WHERE tenant_id = ?1 AND finding_id = ?2",
            vec![tenant_id.to_string().into(), finding_id.to_string().into()],
        ))
        .await
        .unwrap();
        let reopened = writer
            .record_digest_mismatch(&request(tenant_id, scope.clone(), 'd'))
            .await
            .unwrap();
        assert!(matches!(
            reopened,
            IndexDriftFindingWriteOutcome::Reopened { .. }
        ));
        assert_eq!(reopened.finding_id(), finding_id);

        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE index_consistency_findings SET state = 'ignored', closed_at = CURRENT_TIMESTAMP WHERE tenant_id = ?1 AND finding_id = ?2",
            vec![tenant_id.to_string().into(), finding_id.to_string().into()],
        ))
        .await
        .unwrap();
        let suppressed = writer
            .record_digest_mismatch(&request(tenant_id, scope, 'e'))
            .await
            .unwrap();
        assert!(matches!(
            suppressed,
            IndexDriftFindingWriteOutcome::Suppressed { .. }
        ));
        assert_eq!(suppressed.finding_id(), finding_id);

        let row = db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT state, actual_digest, details FROM index_consistency_findings WHERE tenant_id = ?1 AND finding_id = ?2",
                vec![tenant_id.to_string().into(), finding_id.to_string().into()],
            ))
            .await
            .unwrap()
            .unwrap();
        let state: String = row.try_get("", "state").unwrap();
        let actual: String = row.try_get("", "actual_digest").unwrap();
        let details: JsonValue = row.try_get("", "details").unwrap();
        assert_eq!(state, "ignored");
        assert_eq!(actual, "e".repeat(DIGEST_BYTES));
        assert_eq!(details, finding_details());
    }

    #[tokio::test]
    async fn writer_rejects_stored_identity_drift_for_the_same_key() {
        let db = database().await;
        let writer = PostgresIndexDriftFindingWriter::new(db.clone());
        let tenant_id = Uuid::new_v4();
        let request = request(tenant_id, scope(), 'b');
        let created = writer.record_digest_mismatch(&request).await.unwrap();
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE index_consistency_findings SET check_name = 'other_check' WHERE tenant_id = ?1 AND finding_id = ?2",
            vec![tenant_id.to_string().into(), created.finding_id().to_string().into()],
        ))
        .await
        .unwrap();

        assert!(matches!(
            writer.record_digest_mismatch(&request).await,
            Err(IndexDriftFindingWriteError::StoredContractConflict)
        ));
    }
}
