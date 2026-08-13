//! Durable settings ownership for dynamic module artifacts.
//!
//! Static module settings remain a host-manifest contract in `tenant_modules`.
//! Dynamic artifact settings instead resolve one active admitted installation,
//! then address a tenant-private value through that installation's stable data
//! owner and exact settings-instance binding.

use sea_orm::{
    ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait, Value as SqlValue,
};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

use crate::{
    ModuleArtifactDescriptor, ModuleInstallationScope, TenantModuleSettingsRecord,
    artifact_schema::{ArtifactSchemaValidationError, ArtifactSchemaValidatorCache},
    data::configure_tenant_scope,
    installation::acquire_artifact_activation_lock,
};

#[derive(Debug, Error)]
pub(crate) enum ArtifactSettingsStoreError {
    #[error("artifact settings require a non-nil tenant and a non-empty module slug")]
    InvalidIdentity,
    #[error("artifact settings must be a JSON object")]
    InvalidValue,
    #[error("active artifact settings installation is unavailable")]
    InstallationUnavailable,
    #[error("active artifact settings installation is ambiguous")]
    AmbiguousInstallation,
    #[error("active artifact settings installation has invalid immutable metadata")]
    InvalidInstallation,
    #[error("active artifact does not declare a settings schema")]
    MissingSchema,
    #[error("artifact settings do not satisfy the admitted schema")]
    SchemaViolation,
    #[error("artifact settings schema validator is unavailable")]
    ValidatorUnavailable,
    #[error("artifact settings instance schema differs from its active installation")]
    SchemaMismatch,
    #[error("artifact settings instance was purged and cannot be reused")]
    Tombstoned,
    #[error("artifact settings persistence failed: {0}")]
    Database(String),
}

#[derive(Clone)]
struct ActiveSettingsInstallation {
    data_owner_id: Uuid,
    settings_instance_id: Uuid,
    schema_digest: Option<String>,
    schema: Option<Value>,
}

pub(crate) async fn persist(
    db: &DatabaseConnection,
    validators: &ArtifactSchemaValidatorCache,
    tenant_id: Uuid,
    module_slug: &str,
    settings: Value,
) -> Result<TenantModuleSettingsRecord, ArtifactSettingsStoreError> {
    if tenant_id.is_nil() || module_slug.trim().is_empty() || module_slug.trim() != module_slug {
        return Err(ArtifactSettingsStoreError::InvalidIdentity);
    }
    if !settings.is_object() {
        return Err(ArtifactSettingsStoreError::InvalidValue);
    }

    let transaction = db.begin().await.map_err(database_error)?;
    configure_tenant_scope(&transaction, tenant_id)
        .await
        .map_err(|error| ArtifactSettingsStoreError::Database(error.to_string()))?;
    let installation = resolve_active_installation(&transaction, tenant_id, module_slug).await?;
    ensure_not_tombstoned(&transaction, tenant_id, &installation).await?;
    let schema_digest = installation
        .schema_digest
        .as_deref()
        .ok_or(ArtifactSettingsStoreError::MissingSchema)?;
    let schema = installation
        .schema
        .as_ref()
        .ok_or(ArtifactSettingsStoreError::InvalidInstallation)?;
    validators
        .validate(schema_digest, schema, &settings)
        .map_err(map_validation_error)?;
    persist_settings_instance(&transaction, tenant_id, &installation, settings.clone()).await?;
    transaction.commit().await.map_err(database_error)?;

    Ok(TenantModuleSettingsRecord {
        id: installation.settings_instance_id,
        module_slug: module_slug.to_string(),
        enabled: true,
        settings,
    })
}

pub(crate) async fn load(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<Value, ArtifactSettingsStoreError> {
    if tenant_id.is_nil() || module_slug.trim().is_empty() || module_slug.trim() != module_slug {
        return Err(ArtifactSettingsStoreError::InvalidIdentity);
    }

    let transaction = db.begin().await.map_err(database_error)?;
    configure_tenant_scope(&transaction, tenant_id)
        .await
        .map_err(|error| ArtifactSettingsStoreError::Database(error.to_string()))?;
    let installation = resolve_active_installation(&transaction, tenant_id, module_slug).await?;
    ensure_not_tombstoned(&transaction, tenant_id, &installation).await?;
    let Some(expected_schema_digest) = installation.schema_digest.as_deref() else {
        transaction.commit().await.map_err(database_error)?;
        return Ok(serde_json::json!({}));
    };
    let backend = transaction.get_database_backend();
    let row = transaction
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT settings, schema_digest FROM module_artifact_settings_instances \
                 WHERE tenant_id = {} AND data_owner_id = {} AND settings_instance_id = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(installation.data_owner_id, backend),
                uuid_value(installation.settings_instance_id, backend),
            ],
        ))
        .await
        .map_err(database_error)?;
    let settings = match row {
        Some(row) => {
            let stored_schema_digest: String =
                row.try_get("", "schema_digest").map_err(database_error)?;
            if stored_schema_digest != expected_schema_digest {
                return Err(ArtifactSettingsStoreError::SchemaMismatch);
            }
            row.try_get("", "settings").map_err(database_error)?
        }
        None => serde_json::json!({}),
    };
    transaction.commit().await.map_err(database_error)?;
    Ok(settings)
}

async fn resolve_active_installation<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    module_slug: &str,
) -> Result<ActiveSettingsInstallation, ArtifactSettingsStoreError> {
    acquire_artifact_activation_lock(connection, &ModuleInstallationScope::Platform, module_slug)
        .await
        .map_err(|error| ArtifactSettingsStoreError::Database(error.to_string()))?;
    acquire_artifact_activation_lock(
        connection,
        &ModuleInstallationScope::Tenant { tenant_id },
        module_slug,
    )
    .await
    .map_err(|error| ArtifactSettingsStoreError::Database(error.to_string()))?;

    let backend = connection.get_database_backend();
    let rows = connection
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT installation.scope_kind, installation.data_owner_id, \
                 installation.settings_instance_id, CAST(installation.descriptor AS TEXT) AS descriptor \
                 FROM module_artifact_installations installation \
                 JOIN module_artifact_admissions admission \
                   ON admission.installation_id = installation.installation_id \
                 WHERE installation.slug = {} AND admission.status = 'active' \
                   AND NOT EXISTS (SELECT 1 FROM module_artifact_uninstall_operations uninstall \
                                   WHERE uninstall.installation_id = installation.installation_id) \
                   AND ((installation.scope_kind = 'tenant' AND installation.tenant_id = {}) \
                        OR (installation.scope_kind = 'platform' AND installation.tenant_id IS NULL)) \
                 ORDER BY CASE WHEN installation.scope_kind = 'tenant' THEN 0 ELSE 1 END \
                 LIMIT 3",
                placeholder(backend, 1),
                placeholder(backend, 2),
            ),
            vec![module_slug.to_string().into(), uuid_value(tenant_id, backend)],
        ))
        .await
        .map_err(database_error)?;
    let selected = rows
        .first()
        .ok_or(ArtifactSettingsStoreError::InstallationUnavailable)?;
    let selected_scope: String = selected.try_get("", "scope_kind").map_err(database_error)?;
    if rows.iter().skip(1).any(|row| {
        row.try_get::<String>("", "scope_kind")
            .is_ok_and(|scope| scope == selected_scope)
    }) {
        return Err(ArtifactSettingsStoreError::AmbiguousInstallation);
    }
    let data_owner_id = uuid_from_row(selected, "data_owner_id", backend)?;
    let settings_instance_id = uuid_from_row(selected, "settings_instance_id", backend)?;
    if data_owner_id.is_nil() || settings_instance_id.is_nil() {
        return Err(ArtifactSettingsStoreError::InvalidInstallation);
    }
    let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
        &selected
            .try_get::<String>("", "descriptor")
            .map_err(database_error)?,
    )
    .map_err(|_| ArtifactSettingsStoreError::InvalidInstallation)?;
    descriptor
        .validate()
        .map_err(|_| ArtifactSettingsStoreError::InvalidInstallation)?;
    if descriptor.slug != module_slug {
        return Err(ArtifactSettingsStoreError::InvalidInstallation);
    }
    let schema_digest = descriptor.settings_schema_digest.clone();
    let schema = match schema_digest.as_ref() {
        Some(_) => Some(
            descriptor
                .settings_schema()
                .cloned()
                .ok_or(ArtifactSettingsStoreError::InvalidInstallation)?,
        ),
        None => None,
    };
    Ok(ActiveSettingsInstallation {
        data_owner_id,
        settings_instance_id,
        schema_digest,
        schema,
    })
}

async fn persist_settings_instance<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation: &ActiveSettingsInstallation,
    settings: Value,
) -> Result<(), ArtifactSettingsStoreError> {
    let backend = connection.get_database_backend();
    let existing = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT schema_digest FROM module_artifact_settings_instances \
                 WHERE tenant_id = {} AND data_owner_id = {} AND settings_instance_id = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(installation.data_owner_id, backend),
                uuid_value(installation.settings_instance_id, backend),
            ],
        ))
        .await
        .map_err(database_error)?;
    if let Some(existing) = existing {
        let schema_digest: String = existing
            .try_get("", "schema_digest")
            .map_err(database_error)?;
        if Some(schema_digest.as_str()) != installation.schema_digest.as_deref() {
            return Err(ArtifactSettingsStoreError::SchemaMismatch);
        }
        connection
            .execute(Statement::from_sql_and_values(
                backend,
                format!(
                    "UPDATE module_artifact_settings_instances \
                     SET settings = {}, revision = revision + 1, updated_at = {} \
                     WHERE tenant_id = {} AND data_owner_id = {} AND settings_instance_id = {}",
                    placeholder(backend, 1),
                    now_expression(backend),
                    placeholder(backend, 2),
                    placeholder(backend, 3),
                    placeholder(backend, 4),
                ),
                vec![
                    SqlValue::Json(Some(Box::new(settings))),
                    uuid_value(tenant_id, backend),
                    uuid_value(installation.data_owner_id, backend),
                    uuid_value(installation.settings_instance_id, backend),
                ],
            ))
            .await
            .map_err(database_error)?;
        return Ok(());
    }

    connection
        .execute(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_settings_instances \
                 (tenant_id, data_owner_id, settings_instance_id, schema_digest, settings, revision, created_at, updated_at) \
                 VALUES ({}, {}, {}, {}, {}, 1, {}, {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                now_expression(backend),
                now_expression(backend),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(installation.data_owner_id, backend),
                uuid_value(installation.settings_instance_id, backend),
                installation
                    .schema_digest
                    .clone()
                    .ok_or(ArtifactSettingsStoreError::MissingSchema)?
                    .into(),
                SqlValue::Json(Some(Box::new(settings))),
            ],
        ))
        .await
        .map_err(database_error)?;
    Ok(())
}

async fn ensure_not_tombstoned<C: ConnectionTrait>(
    connection: &C,
    tenant_id: Uuid,
    installation: &ActiveSettingsInstallation,
) -> Result<(), ArtifactSettingsStoreError> {
    let backend = connection.get_database_backend();
    let tombstone = connection
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT 1 FROM module_artifact_settings_tombstones WHERE tenant_id = {} AND data_owner_id = {} AND settings_instance_id = {}",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(tenant_id, backend),
                uuid_value(installation.data_owner_id, backend),
                uuid_value(installation.settings_instance_id, backend),
            ],
        ))
        .await
        .map_err(database_error)?;
    if tombstone.is_some() {
        return Err(ArtifactSettingsStoreError::Tombstoned);
    }
    Ok(())
}

fn map_validation_error(error: ArtifactSchemaValidationError) -> ArtifactSettingsStoreError {
    match error {
        ArtifactSchemaValidationError::Compilation => {
            ArtifactSettingsStoreError::InvalidInstallation
        }
        ArtifactSchemaValidationError::Violation => ArtifactSettingsStoreError::SchemaViolation,
        ArtifactSchemaValidationError::CachePoisoned => {
            ArtifactSettingsStoreError::ValidatorUnavailable
        }
    }
}

fn placeholder(backend: DbBackend, index: usize) -> String {
    match backend {
        DbBackend::Postgres => format!("${index}"),
        _ => format!("?{index}"),
    }
}

fn now_expression(backend: DbBackend) -> &'static str {
    match backend {
        DbBackend::Postgres => "NOW()",
        _ => "CURRENT_TIMESTAMP",
    }
}

fn uuid_value(value: Uuid, backend: DbBackend) -> SqlValue {
    match backend {
        DbBackend::Postgres => SqlValue::Uuid(Some(Box::new(value))),
        _ => value.to_string().into(),
    }
}

fn uuid_from_row(
    row: &sea_orm::QueryResult,
    column: &str,
    backend: DbBackend,
) -> Result<Uuid, ArtifactSettingsStoreError> {
    match backend {
        DbBackend::Postgres => row.try_get("", column).map_err(database_error),
        _ => row
            .try_get::<String>("", column)
            .map_err(database_error)?
            .parse()
            .map_err(|_| ArtifactSettingsStoreError::InvalidInstallation),
    }
}

fn database_error(error: impl std::fmt::Display) -> ArtifactSettingsStoreError {
    ArtifactSettingsStoreError::Database(error.to_string())
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TryGetable};

    use super::*;
    use crate::{
        ArtifactModuleKind, ArtifactPayloadKind, ArtifactSchemaDocument,
        MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION, canonical_schema_digest,
    };

    #[tokio::test]
    async fn settings_use_the_active_installation_binding_not_a_slug_row() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        for statement in [
            "CREATE TABLE module_artifact_activation_locks (scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, slug TEXT NOT NULL, PRIMARY KEY (scope_kind, scope_tenant_key, slug))",
            "CREATE TABLE module_artifact_installations (installation_id TEXT PRIMARY KEY, scope_kind TEXT NOT NULL, tenant_id TEXT NULL, slug TEXT NOT NULL, data_owner_id TEXT NOT NULL, settings_instance_id TEXT NOT NULL, descriptor JSON NOT NULL)",
            "CREATE TABLE module_artifact_admissions (installation_id TEXT PRIMARY KEY, status TEXT NOT NULL)",
            "CREATE TABLE module_artifact_uninstall_operations (installation_id TEXT PRIMARY KEY)",
            "CREATE TABLE module_artifact_settings_instances (tenant_id TEXT NOT NULL, data_owner_id TEXT NOT NULL, settings_instance_id TEXT NOT NULL, schema_digest TEXT NOT NULL, settings JSON NOT NULL, revision INTEGER NOT NULL, created_at TEXT NOT NULL, updated_at TEXT NOT NULL, PRIMARY KEY (tenant_id, data_owner_id, settings_instance_id))",
            "CREATE TABLE module_artifact_settings_tombstones (tenant_id TEXT NOT NULL, data_owner_id TEXT NOT NULL, settings_instance_id TEXT NOT NULL, PRIMARY KEY (tenant_id, data_owner_id, settings_instance_id))",
        ] {
            database
                .execute(Statement::from_string(
                    DbBackend::Sqlite,
                    statement.to_string(),
                ))
                .await
                .expect("fixture schema");
        }

        let tenant_id = Uuid::new_v4();
        let installation_id = Uuid::new_v4();
        let data_owner_id = Uuid::new_v4();
        let settings_instance_id = Uuid::new_v4();
        let schema = serde_json::json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": { "theme": { "type": "string" } },
            "required": ["theme"],
            "additionalProperties": false,
        });
        let schema_digest = canonical_schema_digest(&schema);
        let descriptor = ModuleArtifactDescriptor {
            schema_version: MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
            slug: "sample_module".to_string(),
            version: "1.0.0".to_string(),
            payload_kind: ArtifactPayloadKind::Rhai,
            module_kind: ArtifactModuleKind::Optional,
            runtime_abi: "rustok:module/runtime@1".to_string(),
            platform_compatibility: "^0.1".to_string(),
            required_features: Vec::new(),
            artifact_digest: format!("sha256:{}", "a".repeat(64)),
            entrypoint: "main".to_string(),
            capabilities: Vec::new(),
            bindings: Vec::new(),
            dependencies: Vec::new(),
            permissions: Vec::new(),
            schema_documents: vec![ArtifactSchemaDocument {
                digest: schema_digest.clone(),
                document: schema,
            }],
            settings_schema_digest: Some(schema_digest.clone()),
            data_schema_digest: None,
            ui_contributions: Vec::new(),
            persistence_contract: None,
        };
        descriptor.validate().expect("descriptor");
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_installations (installation_id, scope_kind, tenant_id, slug, data_owner_id, settings_instance_id, descriptor) VALUES (?1, 'platform', NULL, 'sample_module', ?2, ?3, ?4)".to_string(),
                vec![
                    installation_id.to_string().into(),
                    data_owner_id.to_string().into(),
                    settings_instance_id.to_string().into(),
                    SqlValue::Json(Some(Box::new(
                        serde_json::to_value(&descriptor).expect("descriptor JSON"),
                    ))),
                ],
            ))
            .await
            .expect("installation");
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_admissions (installation_id, status) VALUES (?1, 'active')".to_string(),
                vec![installation_id.to_string().into()],
            ))
            .await
            .expect("admission");

        let validators = ArtifactSchemaValidatorCache::default();
        let persisted = persist(
            &database,
            &validators,
            tenant_id,
            "sample_module",
            serde_json::json!({ "theme": "dark" }),
        )
        .await
        .expect("persist settings");
        assert_eq!(persisted.id, settings_instance_id);
        assert_eq!(persisted.settings, serde_json::json!({ "theme": "dark" }));
        assert_eq!(
            load(&database, tenant_id, "sample_module")
                .await
                .expect("load settings"),
            serde_json::json!({ "theme": "dark" })
        );
        assert!(matches!(
            persist(
                &database,
                &validators,
                tenant_id,
                "sample_module",
                serde_json::json!({ "theme": 7 }),
            )
            .await,
            Err(ArtifactSettingsStoreError::SchemaViolation)
        ));

        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT data_owner_id, settings_instance_id, schema_digest, revision FROM module_artifact_settings_instances".to_string(),
            ))
            .await
            .expect("query")
            .expect("settings row");
        assert_eq!(
            String::try_get(&row, "", "data_owner_id").expect("data owner"),
            data_owner_id.to_string()
        );
        assert_eq!(
            String::try_get(&row, "", "settings_instance_id").expect("settings instance"),
            settings_instance_id.to_string()
        );
        assert_eq!(
            String::try_get(&row, "", "schema_digest").expect("schema digest"),
            schema_digest
        );
        assert_eq!(i64::try_get(&row, "", "revision").expect("revision"), 1);

        let stateless_installation_id = Uuid::new_v4();
        let mut stateless_descriptor = descriptor;
        stateless_descriptor.slug = "stateless_module".to_string();
        stateless_descriptor.version = "1.0.1".to_string();
        stateless_descriptor.artifact_digest = format!("sha256:{}", "b".repeat(64));
        stateless_descriptor.schema_documents.clear();
        stateless_descriptor.settings_schema_digest = None;
        stateless_descriptor
            .validate()
            .expect("stateless descriptor");
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_installations (installation_id, scope_kind, tenant_id, slug, data_owner_id, settings_instance_id, descriptor) VALUES (?1, 'platform', NULL, 'stateless_module', ?2, ?3, ?4)".to_string(),
                vec![
                    stateless_installation_id.to_string().into(),
                    Uuid::new_v4().to_string().into(),
                    Uuid::new_v4().to_string().into(),
                    SqlValue::Json(Some(Box::new(
                        serde_json::to_value(&stateless_descriptor).expect("descriptor JSON"),
                    ))),
                ],
            ))
            .await
            .expect("stateless installation");
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO module_artifact_admissions (installation_id, status) VALUES (?1, 'active')".to_string(),
                vec![stateless_installation_id.to_string().into()],
            ))
            .await
            .expect("stateless admission");
        assert_eq!(
            load(&database, tenant_id, "stateless_module")
                .await
                .expect("load stateless settings"),
            serde_json::json!({})
        );
        assert!(matches!(
            persist(
                &database,
                &validators,
                tenant_id,
                "stateless_module",
                serde_json::json!({}),
            )
            .await,
            Err(ArtifactSettingsStoreError::MissingSchema)
        ));
    }
}
