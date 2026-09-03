//! Durable RBAC vocabulary for permissions declared by admitted artifacts.

use std::collections::HashSet;

use async_trait::async_trait;
use rustok_api::{
    ArtifactPermissionRegistrationPort, ArtifactPermissionRegistrationRequest,
    ArtifactPermissionScope, PortError, normalize_locale_tag,
};
use sea_orm::{
    ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, Statement,
    TransactionTrait,
};
use uuid::Uuid;

const MAX_PERMISSION_KEY_LENGTH: usize = 256;

/// RBAC-owned durable adapter for admitted artifact permission vocabulary.
///
/// It intentionally writes neither `roles` nor `role_permissions`: registration
/// makes a permission available for later policy assignment, never grants it.
/// Language-neutral identity is persisted once and localized copy is stored in
/// the owner translations table.
#[derive(Clone)]
pub struct RbacArtifactPermissionCatalog {
    db: DatabaseConnection,
}

impl RbacArtifactPermissionCatalog {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl ArtifactPermissionRegistrationPort for RbacArtifactPermissionCatalog {
    async fn register_admitted_permissions(
        &self,
        request: ArtifactPermissionRegistrationRequest,
    ) -> Result<(), PortError> {
        validate_request(&request)?;
        let scope_key = scope_key(&request.scope);
        let backend = self.db.get_database_backend();
        let transaction = self.db.begin().await.map_err(storage_error)?;
        ensure_installation_identity(&transaction, backend, &scope_key, &request).await?;

        for permission in &request.permissions {
            let artifact_permission_id = rustok_core::generate_id();
            transaction
                .execute_raw(Statement::from_sql_and_values(
                    backend,
                    definition_insert_sql(backend)?,
                    vec![
                        artifact_permission_id.into(),
                        scope_key.clone().into(),
                        request.installation_id.into(),
                        request.module_slug.clone().into(),
                        request.release_digest.clone().into(),
                        permission.key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?;

            let definition = transaction
                .query_one_raw(Statement::from_sql_and_values(
                    backend,
                    definition_select_sql(backend)?,
                    vec![
                        scope_key.clone().into(),
                        request.installation_id.into(),
                        permission.key.clone().into(),
                    ],
                ))
                .await
                .map_err(storage_error)?
                .ok_or_else(|| {
                    PortError::invariant_violation(
                        "rbac.artifact_permission_definition_missing",
                        "artifact permission definition disappeared during registration",
                    )
                })?;
            let persisted_id: Uuid = definition.try_get("", "id").map_err(storage_error)?;
            let persisted_module_slug: String = definition
                .try_get("", "module_slug")
                .map_err(storage_error)?;
            let persisted_release_digest: String = definition
                .try_get("", "release_digest")
                .map_err(storage_error)?;
            if persisted_module_slug != request.module_slug
                || persisted_release_digest != request.release_digest
            {
                return Err(PortError::conflict(
                    "rbac.artifact_permission_identity_conflict",
                    "artifact permission identity is already bound to different admitted metadata",
                ));
            }

            for localization in &permission.localizations {
                let normalized_locale =
                    normalize_locale_tag(&localization.locale).ok_or_else(|| {
                        PortError::invariant_violation(
                            "rbac.artifact_permission_locale_not_normalized",
                            "validated artifact permission locale could not be normalized",
                        )
                    })?;
                transaction
                    .execute_raw(Statement::from_sql_and_values(
                        backend,
                        translation_upsert_sql(backend)?,
                        vec![
                            rustok_core::generate_id().into(),
                            persisted_id.into(),
                            normalized_locale.into(),
                            localization.label.clone().into(),
                            localization.description.clone().into(),
                        ],
                    ))
                    .await
                    .map_err(storage_error)?;
            }
        }
        transaction.commit().await.map_err(storage_error)
    }
}

async fn ensure_installation_identity(
    transaction: &DatabaseTransaction,
    backend: DbBackend,
    scope_key: &str,
    request: &ArtifactPermissionRegistrationRequest,
) -> Result<(), PortError> {
    transaction
        .execute_raw(Statement::from_sql_and_values(
            backend,
            installation_insert_sql(backend)?,
            vec![
                request.installation_id.into(),
                scope_key.into(),
                request.module_slug.clone().into(),
                request.release_digest.clone().into(),
            ],
        ))
        .await
        .map_err(storage_error)?;
    let installation = transaction
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            installation_select_sql(backend)?,
            vec![request.installation_id.into()],
        ))
        .await
        .map_err(storage_error)?
        .ok_or_else(|| {
            PortError::invariant_violation(
                "rbac.artifact_permission_installation_missing",
                "artifact permission installation identity disappeared during registration",
            )
        })?;
    let persisted_scope_key: String = installation
        .try_get("", "scope_key")
        .map_err(storage_error)?;
    let persisted_module_slug: String = installation
        .try_get("", "module_slug")
        .map_err(storage_error)?;
    let persisted_release_digest: String = installation
        .try_get("", "release_digest")
        .map_err(storage_error)?;
    if persisted_scope_key != scope_key
        || persisted_module_slug != request.module_slug
        || persisted_release_digest != request.release_digest
    {
        return Err(PortError::conflict(
            "rbac.artifact_permission_identity_conflict",
            "artifact installation identity is already bound to different admitted scope or metadata",
        ));
    }
    Ok(())
}

fn storage_error(error: impl std::fmt::Display) -> PortError {
    PortError::unavailable("rbac.artifact_permission_catalog", error.to_string())
}

fn validate_request(request: &ArtifactPermissionRegistrationRequest) -> Result<(), PortError> {
    if request.installation_id.is_nil()
        || matches!(
            &request.scope,
            ArtifactPermissionScope::Tenant { tenant_id } if tenant_id.is_nil()
        )
        || request.module_slug.trim().is_empty()
        || request.release_digest.trim().is_empty()
        || request.permissions.is_empty()
    {
        return Err(PortError::validation(
            "rbac.artifact_permission_registration_invalid",
            "artifact permission registration requires immutable installation identity and permissions",
        ));
    }
    let prefix = format!("{}.", request.module_slug);
    let mut permission_keys = HashSet::new();
    for permission in &request.permissions {
        if !permission.key.starts_with(&prefix)
            || permission.key.len() > MAX_PERMISSION_KEY_LENGTH
            || permission.key.trim() != permission.key
            || permission.key.chars().any(char::is_control)
            || !permission_keys.insert(permission.key.as_str())
            || permission.localizations.is_empty()
        {
            return Err(PortError::validation(
                "rbac.artifact_permission_registration_invalid",
                "artifact permissions must remain module-owned, bounded, unique and localized",
            ));
        }
        let mut normalized_locales = HashSet::new();
        for localization in &permission.localizations {
            let Some(normalized_locale) = normalize_locale_tag(&localization.locale) else {
                return Err(PortError::validation(
                    "rbac.artifact_permission_registration_invalid",
                    "artifact permission localizations must use valid canonical locale tags",
                ));
            };
            if localization.label.trim().is_empty()
                || localization.description.trim().is_empty()
                || !normalized_locales.insert(normalized_locale)
            {
                return Err(PortError::validation(
                    "rbac.artifact_permission_registration_invalid",
                    "artifact permission localizations must be non-empty and unique after locale normalization",
                ));
            }
        }
    }
    Ok(())
}

fn scope_key(scope: &ArtifactPermissionScope) -> String {
    match scope {
        ArtifactPermissionScope::Platform => "platform".to_string(),
        ArtifactPermissionScope::Tenant { tenant_id } => format!("tenant:{tenant_id}"),
    }
}

fn installation_insert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest) VALUES (?1, ?2, ?3, ?4) ON CONFLICT (installation_id) DO NOTHING",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_permission_installations (installation_id, scope_key, module_slug, release_digest) VALUES ($1, $2, $3, $4) ON CONFLICT (installation_id) DO NOTHING",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn installation_select_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "SELECT scope_key, module_slug, release_digest FROM rbac_artifact_permission_installations WHERE installation_id = ?1 LIMIT 1",
        ),
        DbBackend::Postgres => Ok(
            "SELECT scope_key, module_slug, release_digest FROM rbac_artifact_permission_installations WHERE installation_id = $1 LIMIT 1",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn definition_insert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_permission_definitions (id, scope_key, installation_id, module_slug, release_digest, permission_key) VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT (scope_key, installation_id, permission_key) DO NOTHING",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_permission_definitions (id, scope_key, installation_id, module_slug, release_digest, permission_key) VALUES ($1, $2, $3, $4, $5, $6) ON CONFLICT (scope_key, installation_id, permission_key) DO NOTHING",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn definition_select_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "SELECT id, module_slug, release_digest FROM rbac_artifact_permission_definitions WHERE scope_key = ?1 AND installation_id = ?2 AND permission_key = ?3 LIMIT 1",
        ),
        DbBackend::Postgres => Ok(
            "SELECT id, module_slug, release_digest FROM rbac_artifact_permission_definitions WHERE scope_key = $1 AND installation_id = $2 AND permission_key = $3 LIMIT 1",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

fn translation_upsert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_permission_translations (id, artifact_permission_id, locale, label, description) VALUES (?1, ?2, ?3, ?4, ?5) ON CONFLICT (artifact_permission_id, locale) DO UPDATE SET label = excluded.label, description = excluded.description",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_permission_translations (id, artifact_permission_id, locale, label, description) VALUES ($1, $2, $3, $4, $5) ON CONFLICT (artifact_permission_id, locale) DO UPDATE SET label = EXCLUDED.label, description = EXCLUDED.description",
        ),
        backend => Err(PortError::validation(
            "rbac.artifact_permission_backend_unsupported",
            format!("artifact permission catalog does not support {backend:?}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::{ArtifactPermissionLocalization, ArtifactPermissionRegistration};
    use sea_orm::{ConnectionTrait, Database, DatabaseConnection};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    fn request(installation_id: Uuid) -> ArtifactPermissionRegistrationRequest {
        ArtifactPermissionRegistrationRequest {
            installation_id,
            scope: ArtifactPermissionScope::Platform,
            module_slug: "sample_module".to_string(),
            release_digest: format!("sha256:{}", "a".repeat(64)),
            permissions: vec![ArtifactPermissionRegistration {
                key: "sample_module.events.handle".to_string(),
                localizations: vec![ArtifactPermissionLocalization {
                    locale: "en".to_string(),
                    label: "Handle event".to_string(),
                    description: "Allows handling an admitted event".to_string(),
                }],
            }],
        }
    }

    async fn migrate_catalog(database: &DatabaseConnection) {
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .expect("enable SQLite foreign keys");
        for statement in [
            "CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
            "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
        ] {
            database
                .execute_unprepared(statement)
                .await
                .expect("create RBAC catalog parent table");
        }
        let manager = SchemaManager::new(database);
        super::super::m20260716_000001_artifact_permission_catalog::Migration
            .up(&manager)
            .await
            .expect("apply legacy catalog migration");
        super::super::m20260717_000001_artifact_role_permissions::Migration
            .up(&manager)
            .await
            .expect("apply legacy artifact grant migration");
        super::super::m20260803_000001_canonicalize_artifact_permissions::Migration
            .up(&manager)
            .await
            .expect("apply canonical artifact permission cutover");
    }

    #[tokio::test]
    async fn registration_normalizes_locale_and_is_idempotent() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        migrate_catalog(&database).await;
        let catalog = RbacArtifactPermissionCatalog::new(database.clone());
        let installation_id = Uuid::new_v4();
        let mut initial = request(installation_id);
        initial.permissions[0].localizations[0].locale = "EN_us".to_string();
        let mut retry = request(installation_id);
        retry.permissions[0].localizations[0].locale = "en-US".to_string();

        catalog
            .register_admitted_permissions(initial)
            .await
            .expect("initial registration");
        catalog
            .register_admitted_permissions(retry)
            .await
            .expect("canonical idempotent retry");

        for (table, expected) in [
            ("rbac_artifact_permission_definitions", 1_i64),
            ("rbac_artifact_permission_translations", 1_i64),
        ] {
            let row = database
                .query_one_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("SELECT COUNT(*) AS count FROM {table}"),
                ))
                .await
                .expect("catalog query")
                .expect("catalog row");
            let count: i64 = row.try_get("", "count").expect("count");
            assert_eq!(count, expected);
        }
        let locale: String = database
            .query_one_raw(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT locale FROM rbac_artifact_permission_translations LIMIT 1".to_string(),
            ))
            .await
            .expect("translation query")
            .expect("translation row")
            .try_get("", "locale")
            .expect("locale");
        assert_eq!(locale, "en-US");
    }

    #[tokio::test]
    async fn registration_rejects_identity_rebinding() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        migrate_catalog(&database).await;
        let catalog = RbacArtifactPermissionCatalog::new(database);
        let installation_id = Uuid::new_v4();
        catalog
            .register_admitted_permissions(request(installation_id))
            .await
            .expect("initial registration");

        let mut conflicting = request(installation_id);
        conflicting.release_digest = format!("sha256:{}", "b".repeat(64));
        let error = catalog
            .register_admitted_permissions(conflicting)
            .await
            .expect_err("identity rebinding must fail closed");
        assert_eq!(error.code, "rbac.artifact_permission_identity_conflict");
    }

    #[tokio::test]
    async fn registration_rejects_installation_scope_rebinding() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        migrate_catalog(&database).await;
        let catalog = RbacArtifactPermissionCatalog::new(database.clone());
        let installation_id = Uuid::new_v4();
        catalog
            .register_admitted_permissions(request(installation_id))
            .await
            .expect("initial platform registration");

        let mut conflicting = request(installation_id);
        conflicting.scope = ArtifactPermissionScope::Tenant {
            tenant_id: Uuid::new_v4(),
        };
        let error = catalog
            .register_admitted_permissions(conflicting)
            .await
            .expect_err("installation scope rebinding must fail closed");
        assert_eq!(error.code, "rbac.artifact_permission_identity_conflict");

        for (table, expected) in [
            ("rbac_artifact_permission_installations", 1_i64),
            ("rbac_artifact_permission_definitions", 1_i64),
        ] {
            let row = database
                .query_one_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    format!("SELECT COUNT(*) AS count FROM {table}"),
                ))
                .await
                .expect("identity count query")
                .expect("identity count row");
            let count: i64 = row.try_get("", "count").expect("identity count");
            assert_eq!(count, expected);
        }
    }

    #[tokio::test]
    async fn registration_rejects_nil_tenant_scope() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        let catalog = RbacArtifactPermissionCatalog::new(database);
        let mut request = request(Uuid::new_v4());
        request.scope = ArtifactPermissionScope::Tenant {
            tenant_id: Uuid::nil(),
        };

        let error = catalog
            .register_admitted_permissions(request)
            .await
            .expect_err("nil tenant scope must fail closed");
        assert_eq!(error.code, "rbac.artifact_permission_registration_invalid");
    }

    #[tokio::test]
    async fn registration_rejects_duplicate_normalized_locales() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        let catalog = RbacArtifactPermissionCatalog::new(database);
        let mut request = request(Uuid::new_v4());
        request.permissions[0].localizations[0].locale = "en-US".to_string();
        request.permissions[0]
            .localizations
            .push(ArtifactPermissionLocalization {
                locale: "EN_us".to_string(),
                label: "Duplicate handle event".to_string(),
                description: "Duplicate normalized locale".to_string(),
            });

        let error = catalog
            .register_admitted_permissions(request)
            .await
            .expect_err("duplicate normalized locales must fail closed");
        assert_eq!(error.code, "rbac.artifact_permission_registration_invalid");
    }

    #[tokio::test]
    async fn registration_rejects_unassignable_or_duplicate_permission_keys() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        let catalog = RbacArtifactPermissionCatalog::new(database);

        for invalid_key in [
            "sample_module.events.handle ".to_string(),
            "sample_module.events.\nhandle".to_string(),
            format!("sample_module.{}", "x".repeat(MAX_PERMISSION_KEY_LENGTH)),
        ] {
            let mut invalid = request(Uuid::new_v4());
            invalid.permissions[0].key = invalid_key;
            let error = catalog
                .register_admitted_permissions(invalid)
                .await
                .expect_err("unassignable permission key must fail registration");
            assert_eq!(error.code, "rbac.artifact_permission_registration_invalid");
        }

        let mut duplicate = request(Uuid::new_v4());
        duplicate.permissions.push(duplicate.permissions[0].clone());
        let error = catalog
            .register_admitted_permissions(duplicate)
            .await
            .expect_err("duplicate permission key must fail registration");
        assert_eq!(error.code, "rbac.artifact_permission_registration_invalid");
    }

    #[tokio::test]
    async fn registration_rejects_a_permission_outside_the_module_namespace() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        let catalog = RbacArtifactPermissionCatalog::new(database);
        let mut request = request(Uuid::new_v4());
        request.permissions[0].key = "other_module.events.handle".to_string();

        let error = catalog
            .register_admitted_permissions(request)
            .await
            .expect_err("foreign permission namespace must be rejected");
        assert_eq!(error.code, "rbac.artifact_permission_registration_invalid");
    }
}
