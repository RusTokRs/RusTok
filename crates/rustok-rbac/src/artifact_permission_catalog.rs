//! Durable RBAC vocabulary for permissions declared by admitted artifacts.

use async_trait::async_trait;
use rustok_api::{
    ArtifactPermissionRegistrationPort, ArtifactPermissionRegistrationRequest,
    ArtifactPermissionScope, PortError,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, TransactionTrait};

/// RBAC-owned durable adapter for admitted artifact permission vocabulary.
///
/// It intentionally writes neither `roles` nor `role_permissions`: registration
/// makes a permission available for later policy assignment, never grants it.
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
        let sql = upsert_sql(backend)?;
        let transaction = self.db.begin().await.map_err(|error| {
            PortError::unavailable("rbac.artifact_permission_catalog", error.to_string())
        })?;
        for permission in &request.permissions {
            for localization in &permission.localizations {
                transaction
                    .execute(Statement::from_sql_and_values(
                        backend,
                        sql,
                        vec![
                            rustok_core::generate_id().into(),
                            scope_key.clone().into(),
                            request.installation_id.into(),
                            request.module_slug.clone().into(),
                            request.release_digest.clone().into(),
                            permission.key.clone().into(),
                            localization.locale.clone().into(),
                            localization.label.clone().into(),
                            localization.description.clone().into(),
                        ],
                    ))
                    .await
                    .map_err(|error| {
                        PortError::unavailable(
                            "rbac.artifact_permission_catalog",
                            error.to_string(),
                        )
                    })?;
            }
        }
        transaction.commit().await.map_err(|error| {
            PortError::unavailable("rbac.artifact_permission_catalog", error.to_string())
        })
    }
}

fn validate_request(request: &ArtifactPermissionRegistrationRequest) -> Result<(), PortError> {
    if request.installation_id.is_nil()
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
    for permission in &request.permissions {
        if !permission.key.starts_with(&prefix) || permission.localizations.is_empty() {
            return Err(PortError::validation(
                "rbac.artifact_permission_registration_invalid",
                "artifact permissions must remain module-owned and localized",
            ));
        }
        for (index, localization) in permission.localizations.iter().enumerate() {
            if localization.locale.trim().is_empty()
                || localization.label.trim().is_empty()
                || localization.description.trim().is_empty()
                || permission.localizations[..index]
                    .iter()
                    .any(|previous| previous.locale == localization.locale)
            {
                return Err(PortError::validation(
                    "rbac.artifact_permission_registration_invalid",
                    "artifact permission localizations must be non-empty and unique by locale",
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

fn upsert_sql(backend: DbBackend) -> Result<&'static str, PortError> {
    match backend {
        DbBackend::Sqlite => Ok(
            "INSERT INTO rbac_artifact_permission_catalog (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9) ON CONFLICT (scope_key, installation_id, permission_key, locale) DO UPDATE SET label = excluded.label, description = excluded.description",
        ),
        DbBackend::Postgres => Ok(
            "INSERT INTO rbac_artifact_permission_catalog (id, scope_key, installation_id, module_slug, release_digest, permission_key, locale, label, description) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9) ON CONFLICT (scope_key, installation_id, permission_key, locale) DO UPDATE SET label = EXCLUDED.label, description = EXCLUDED.description",
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
    use sea_orm::{ConnectionTrait, Database};
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
    use uuid::Uuid;

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

    #[tokio::test]
    async fn registration_is_idempotent_and_does_not_require_role_tables() {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("database");
        super::super::m20260716_000001_artifact_permission_catalog::Migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("catalog migration");
        let catalog = RbacArtifactPermissionCatalog::new(database.clone());
        let installation_id = Uuid::new_v4();

        catalog
            .register_admitted_permissions(request(installation_id))
            .await
            .expect("initial registration");
        catalog
            .register_admitted_permissions(request(installation_id))
            .await
            .expect("idempotent retry");

        let row = database
            .query_one(Statement::from_string(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS count FROM rbac_artifact_permission_catalog".to_string(),
            ))
            .await
            .expect("catalog query")
            .expect("catalog row");
        let count: i64 = row.try_get("", "count").expect("count");
        assert_eq!(count, 1);
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
