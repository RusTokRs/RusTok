use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, QueryResult, Statement, Value as SqlValue,
};
use thiserror::Error;
use uuid::Uuid;

use crate::{EntityName, LocaleKey, ModuleName, SchemaRef, SchemaVersion};

const DIGEST_BYTES: usize = 64;
const MAX_CHECK_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexDriftFindingSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IndexDriftFindingScope {
    Global,
    Schema {
        schema: SchemaRef,
    },
    Entity {
        schema: SchemaRef,
        entity_id: Uuid,
        locale: LocaleKey,
    },
    EntityWithoutLocale {
        schema: SchemaRef,
        entity_id: Uuid,
    },
}

/// Bounded read-only diagnosis of one open consistency finding.
///
/// The result intentionally excludes tenant identity, raw `details`, detection
/// timestamps, and closure state. It exposes only stable finding identity, bounded
/// classification, exact typed scope, and optional digest evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexDriftFindingInspection {
    finding_id: Uuid,
    finding_key: String,
    check_name: String,
    severity: IndexDriftFindingSeverity,
    scope: IndexDriftFindingScope,
    expected_digest: Option<String>,
    actual_digest: Option<String>,
}

impl IndexDriftFindingInspection {
    pub fn finding_id(&self) -> Uuid {
        self.finding_id
    }

    pub fn finding_key(&self) -> &str {
        &self.finding_key
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

    pub fn expected_digest(&self) -> Option<&str> {
        self.expected_digest.as_deref()
    }

    pub fn actual_digest(&self) -> Option<&str> {
        self.actual_digest.as_deref()
    }
}

/// PostgreSQL-backed read-only inspector for one exact open drift finding.
///
/// Authorization is deliberately not accepted as adapter data. A server or other
/// operator boundary must authorize the exact tenant before calling `inspect`.
#[derive(Clone)]
pub struct PostgresIndexDriftFindingInspector {
    db: DatabaseConnection,
}

impl PostgresIndexDriftFindingInspector {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub async fn inspect(
        &self,
        tenant_id: Uuid,
        finding_id: Uuid,
    ) -> Result<Option<IndexDriftFindingInspection>, IndexDriftFindingInspectionError> {
        if tenant_id.is_nil() {
            return Err(IndexDriftFindingInspectionError::NilTenantId);
        }
        if finding_id.is_nil() {
            return Err(IndexDriftFindingInspectionError::NilFindingId);
        }

        let backend = self.db.get_database_backend();
        ensure_supported_backend(backend)?;
        let row = self
            .db
            .query_one(Statement::from_sql_and_values(
                backend,
                select_open_finding_sql(backend),
                vec![
                    uuid_value(tenant_id, backend),
                    uuid_value(finding_id, backend),
                ],
            ))
            .await
            .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
        row.map(|row| decode_open_finding(&row, finding_id, backend))
            .transpose()
    }
}

fn decode_open_finding(
    row: &QueryResult,
    finding_id: Uuid,
    backend: DbBackend,
) -> Result<IndexDriftFindingInspection, IndexDriftFindingInspectionError> {
    let finding_key: String = row
        .try_get("", "finding_key")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    validate_digest(&finding_key).map_err(|_| {
        IndexDriftFindingInspectionError::InvalidStoredFinding(
            "finding key is outside the lowercase SHA-256 contract",
        )
    })?;

    let check_name: String = row
        .try_get("", "check_name")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    validate_check_name(&check_name).map_err(|_| {
        IndexDriftFindingInspectionError::InvalidStoredFinding(
            "check name is outside the bounded text contract",
        )
    })?;

    let severity: String = row
        .try_get("", "severity")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    let severity = match severity.as_str() {
        "info" => IndexDriftFindingSeverity::Info,
        "warning" => IndexDriftFindingSeverity::Warning,
        "error" => IndexDriftFindingSeverity::Error,
        _ => {
            return Err(IndexDriftFindingInspectionError::InvalidStoredFinding(
                "severity is unsupported",
            ));
        }
    };

    let scope_kind: String = row
        .try_get("", "scope_kind")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    let module_name: Option<String> = row
        .try_get("", "module_name")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    let entity_name: Option<String> = row
        .try_get("", "entity_name")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    let schema_version: Option<i64> = row
        .try_get("", "schema_version_value")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    let entity_id = optional_uuid(row, "entity_id", backend)?;
    let locale_key: Option<String> = row
        .try_get("", "locale_key")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    let scope = decode_scope(
        &scope_kind,
        module_name,
        entity_name,
        schema_version,
        entity_id,
        locale_key,
    )?;

    let expected_digest: Option<String> = row
        .try_get("", "expected_digest")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    validate_optional_digest(expected_digest.as_deref(), "expected digest")?;
    let actual_digest: Option<String> = row
        .try_get("", "actual_digest")
        .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
    validate_optional_digest(actual_digest.as_deref(), "actual digest")?;

    Ok(IndexDriftFindingInspection {
        finding_id,
        finding_key,
        check_name,
        severity,
        scope,
        expected_digest,
        actual_digest,
    })
}

fn decode_scope(
    scope_kind: &str,
    module_name: Option<String>,
    entity_name: Option<String>,
    schema_version: Option<i64>,
    entity_id: Option<Uuid>,
    locale_key: Option<String>,
) -> Result<IndexDriftFindingScope, IndexDriftFindingInspectionError> {
    match scope_kind {
        "global" => {
            if module_name.is_some()
                || entity_name.is_some()
                || schema_version.is_some()
                || entity_id.is_some()
                || locale_key.is_some()
            {
                return Err(invalid_scope());
            }
            Ok(IndexDriftFindingScope::Global)
        }
        "schema" => {
            if entity_id.is_some() || locale_key.is_some() {
                return Err(invalid_scope());
            }
            Ok(IndexDriftFindingScope::Schema {
                schema: decode_schema(module_name, entity_name, schema_version)?,
            })
        }
        "entity" => {
            let schema = decode_schema(module_name, entity_name, schema_version)?;
            let entity_id = entity_id
                .filter(|value| !value.is_nil())
                .ok_or_else(invalid_scope)?;
            match locale_key {
                Some(stored_locale) => {
                    let locale = LocaleKey::new(&stored_locale).map_err(|_| invalid_scope())?;
                    if locale.as_str() != stored_locale {
                        return Err(invalid_scope());
                    }
                    Ok(IndexDriftFindingScope::Entity {
                        schema,
                        entity_id,
                        locale,
                    })
                }
                None => Ok(IndexDriftFindingScope::EntityWithoutLocale { schema, entity_id }),
            }
        }
        _ => Err(invalid_scope()),
    }
}

fn decode_schema(
    module_name: Option<String>,
    entity_name: Option<String>,
    schema_version: Option<i64>,
) -> Result<SchemaRef, IndexDriftFindingInspectionError> {
    let module =
        ModuleName::new(module_name.ok_or_else(invalid_scope)?).map_err(|_| invalid_scope())?;
    let entity =
        EntityName::new(entity_name.ok_or_else(invalid_scope)?).map_err(|_| invalid_scope())?;
    let version = schema_version.ok_or_else(invalid_scope)?;
    let version = u32::try_from(version).map_err(|_| invalid_scope())?;
    if version == 0 {
        return Err(invalid_scope());
    }
    Ok(SchemaRef {
        module,
        entity,
        version: SchemaVersion::new(version),
    })
}

fn validate_check_name(value: &str) -> Result<(), ()> {
    if value.is_empty()
        || value.len() > MAX_CHECK_NAME_BYTES
        || value.trim() != value
        || value.chars().any(char::is_control)
    {
        return Err(());
    }
    Ok(())
}

fn validate_optional_digest(
    value: Option<&str>,
    label: &'static str,
) -> Result<(), IndexDriftFindingInspectionError> {
    if value.is_some_and(|value| validate_digest(value).is_err()) {
        return Err(IndexDriftFindingInspectionError::InvalidStoredFinding(
            match label {
                "expected digest" => "expected digest is outside the lowercase SHA-256 contract",
                _ => "actual digest is outside the lowercase SHA-256 contract",
            },
        ));
    }
    Ok(())
}

fn validate_digest(value: &str) -> Result<(), ()> {
    if value.len() != DIGEST_BYTES
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(());
    }
    Ok(())
}

fn invalid_scope() -> IndexDriftFindingInspectionError {
    IndexDriftFindingInspectionError::InvalidStoredFinding(
        "finding scope does not match the persisted scope contract",
    )
}

fn ensure_supported_backend(backend: DbBackend) -> Result<(), IndexDriftFindingInspectionError> {
    match backend {
        DbBackend::Postgres => Ok(()),
        DbBackend::Sqlite if cfg!(test) => Ok(()),
        _ => Err(IndexDriftFindingInspectionError::UnsupportedBackend),
    }
}

fn placeholder_prefix(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "$",
        DbBackend::Sqlite => "?",
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => value.into(),
        DbBackend::Sqlite => value.to_string().into(),
        _ => unreachable!("unsupported database backend was validated"),
    }
}

fn optional_uuid(
    row: &QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Option<Uuid>, IndexDriftFindingInspectionError> {
    match backend {
        DbBackend::Postgres => row
            .try_get("", column)
            .map_err(|_| IndexDriftFindingInspectionError::Storage),
        DbBackend::Sqlite => {
            let value: Option<String> = row
                .try_get("", column)
                .map_err(|_| IndexDriftFindingInspectionError::Storage)?;
            value
                .map(|value| Uuid::parse_str(&value).map_err(|_| invalid_scope()))
                .transpose()
        }
        _ => Err(IndexDriftFindingInspectionError::UnsupportedBackend),
    }
}

fn select_open_finding_sql(backend: DbBackend) -> String {
    let prefix = placeholder_prefix(backend);
    let schema_version = match backend {
        DbBackend::Postgres => "CAST(schema_version AS BIGINT)",
        DbBackend::Sqlite => "CAST(schema_version AS INTEGER)",
        _ => unreachable!("unsupported database backend was validated"),
    };
    format!(
        "SELECT finding_key, check_name, severity, scope_kind, module_name, entity_name, {schema_version} AS schema_version_value, entity_id, locale_key, expected_digest, actual_digest FROM index_consistency_findings WHERE tenant_id = {prefix}1 AND finding_id = {prefix}2 AND state = 'open' LIMIT 1"
    )
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IndexDriftFindingInspectionError {
    #[error("Index drift finding inspection tenant id must not be nil")]
    NilTenantId,
    #[error("Index drift finding inspection finding id must not be nil")]
    NilFindingId,
    #[error("stored Index drift finding is invalid: {0}")]
    InvalidStoredFinding(&'static str),
    #[error("Index drift finding inspection does not support this database backend")]
    UnsupportedBackend,
    #[error("Index drift finding inspection storage operation failed")]
    Storage,
}

#[cfg(test)]
mod tests {
    use sea_orm::{Database, DbBackend, Statement};
    use serde_json::json;

    use super::*;

    async fn database() -> DatabaseConnection {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("test database");
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            "CREATE TABLE index_consistency_findings (tenant_id TEXT NOT NULL, finding_id TEXT NOT NULL, finding_key TEXT NOT NULL, check_name TEXT NOT NULL, severity TEXT NOT NULL, state TEXT NOT NULL, scope_kind TEXT NOT NULL, module_name TEXT NULL, entity_name TEXT NULL, schema_version INTEGER NULL, entity_id TEXT NULL, locale_key TEXT NULL, expected_digest TEXT NULL, actual_digest TEXT NULL, details JSON NOT NULL)"
                .to_owned(),
        ))
        .await
        .expect("finding fixture");
        db
    }

    async fn insert_entity_finding(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        finding_id: Uuid,
        state: &str,
        actual_digest: &str,
    ) {
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_consistency_findings (tenant_id, finding_id, finding_key, check_name, severity, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, expected_digest, actual_digest, details) VALUES (?1, ?2, ?3, ?4, 'error', ?5, 'entity', 'rustok-product', 'product', 2, ?6, 'en-US', ?7, ?8, ?9)",
            vec![
                tenant_id.to_string().into(),
                finding_id.to_string().into(),
                "a".repeat(DIGEST_BYTES).into(),
                "entity_digest_mismatch".into(),
                state.into(),
                Uuid::from_u128(7).to_string().into(),
                "b".repeat(DIGEST_BYTES).into(),
                actual_digest.into(),
                SqlValue::Json(Some(Box::new(json!({
                    "private_owner_detail": "must-not-cross-inspection-boundary"
                })))),
            ],
        ))
        .await
        .expect("finding insert");
    }

    #[tokio::test]
    async fn inspection_is_tenant_scoped_open_and_bounded() {
        let db = database().await;
        let tenant_id = Uuid::new_v4();
        let finding_id = Uuid::new_v4();
        insert_entity_finding(
            &db,
            tenant_id,
            finding_id,
            "open",
            &"c".repeat(DIGEST_BYTES),
        )
        .await;
        let inspector = PostgresIndexDriftFindingInspector::new(db.clone());

        assert!(
            inspector
                .inspect(Uuid::new_v4(), finding_id)
                .await
                .unwrap()
                .is_none()
        );
        let inspection = inspector
            .inspect(tenant_id, finding_id)
            .await
            .unwrap()
            .expect("exact open finding");
        assert_eq!(inspection.finding_id(), finding_id);
        assert_eq!(inspection.finding_key(), "a".repeat(DIGEST_BYTES));
        assert_eq!(inspection.check_name(), "entity_digest_mismatch");
        assert_eq!(inspection.severity(), IndexDriftFindingSeverity::Error);
        assert_eq!(
            inspection.expected_digest(),
            Some("b".repeat(DIGEST_BYTES).as_str())
        );
        assert_eq!(
            inspection.actual_digest(),
            Some("c".repeat(DIGEST_BYTES).as_str())
        );
        match inspection.scope() {
            IndexDriftFindingScope::Entity {
                schema,
                entity_id,
                locale,
            } => {
                assert_eq!(schema.module.as_str(), "rustok-product");
                assert_eq!(schema.entity.as_str(), "product");
                assert_eq!(schema.version.get(), 2);
                assert_eq!(*entity_id, Uuid::from_u128(7));
                assert_eq!(locale.as_str(), "en-US");
            }
            other => panic!("unexpected scope: {other:?}"),
        }

        let resolved_id = Uuid::new_v4();
        insert_entity_finding(
            &db,
            tenant_id,
            resolved_id,
            "resolved",
            &"d".repeat(DIGEST_BYTES),
        )
        .await;
        assert!(
            inspector
                .inspect(tenant_id, resolved_id)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn inspection_fails_closed_on_invalid_digest() {
        let db = database().await;
        let tenant_id = Uuid::new_v4();
        let finding_id = Uuid::new_v4();
        insert_entity_finding(&db, tenant_id, finding_id, "open", "not-a-digest").await;
        let inspector = PostgresIndexDriftFindingInspector::new(db);

        assert!(matches!(
            inspector.inspect(tenant_id, finding_id).await,
            Err(IndexDriftFindingInspectionError::InvalidStoredFinding(_))
        ));
    }

    #[tokio::test]
    async fn inspection_fails_closed_on_scope_mismatch() {
        let db = database().await;
        let tenant_id = Uuid::new_v4();
        let finding_id = Uuid::new_v4();
        db.execute(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO index_consistency_findings (tenant_id, finding_id, finding_key, check_name, severity, state, scope_kind, module_name, entity_name, schema_version, entity_id, locale_key, expected_digest, actual_digest, details) VALUES (?1, ?2, ?3, 'scope_mismatch', 'warning', 'open', 'schema', 'rustok-product', 'product', 1, ?4, NULL, NULL, NULL, ?5)",
            vec![
                tenant_id.to_string().into(),
                finding_id.to_string().into(),
                "e".repeat(DIGEST_BYTES).into(),
                Uuid::new_v4().to_string().into(),
                SqlValue::Json(Some(Box::new(json!({})))),
            ],
        ))
        .await
        .expect("invalid scope fixture");
        let inspector = PostgresIndexDriftFindingInspector::new(db);

        assert!(matches!(
            inspector.inspect(tenant_id, finding_id).await,
            Err(IndexDriftFindingInspectionError::InvalidStoredFinding(_))
        ));
    }
}
