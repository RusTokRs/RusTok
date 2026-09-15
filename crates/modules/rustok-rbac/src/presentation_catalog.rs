use rustok_api::{
    PLATFORM_FALLBACK_LOCALE, RuntimeLocale, StoredLocale, TenantRbacCatalog,
    TenantRbacPermission, TenantRbacRole, build_locale_candidates,
};
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use crate::{
    BuiltinTenantRbacCatalog, RbacPresentationResourceKind, RbacPresentationStore,
    RbacPresentationStoreError, SeaOrmRbacPresentationStore,
};

const BUILTIN_ROLE_PRESENTATIONS: &[(&str, &str)] = &[
    ("super_admin", "Super Admin"),
    ("admin", "Admin"),
    ("manager", "Manager"),
    ("customer", "Customer"),
];

#[derive(Clone)]
pub struct RbacLocalizedCatalogReader {
    store: SeaOrmRbacPresentationStore,
}

impl RbacLocalizedCatalogReader {
    pub fn new(db: DatabaseConnection) -> Self {
        Self {
            store: SeaOrmRbacPresentationStore::new(db),
        }
    }

    pub async fn roles(
        &self,
        tenant_id: Uuid,
        locale: &RuntimeLocale,
    ) -> Result<Vec<TenantRbacRole>, RbacPresentationStoreError> {
        let catalog = BuiltinTenantRbacCatalog;
        let mut roles = catalog.roles(tenant_id);
        for role in &mut roles {
            role.display_name = self
                .resolve_name(
                    tenant_id,
                    RbacPresentationResourceKind::Role,
                    &role.slug,
                    locale,
                )
                .await?;
        }
        Ok(roles)
    }

    pub async fn permissions(
        &self,
        tenant_id: Uuid,
        locale: &RuntimeLocale,
    ) -> Result<Vec<TenantRbacPermission>, RbacPresentationStoreError> {
        let catalog = BuiltinTenantRbacCatalog;
        let mut permissions = catalog.permissions(tenant_id);
        for permission in &mut permissions {
            permission.display_name = self
                .resolve_name(
                    tenant_id,
                    RbacPresentationResourceKind::Permission,
                    &permission.slug,
                    locale,
                )
                .await?;
        }
        Ok(permissions)
    }

    async fn resolve_name(
        &self,
        tenant_id: Uuid,
        resource_kind: RbacPresentationResourceKind,
        resource_key: &str,
        locale: &RuntimeLocale,
    ) -> Result<String, RbacPresentationStoreError> {
        let candidates = build_locale_candidates(
            [Some(locale.as_str()), Some(PLATFORM_FALLBACK_LOCALE)],
            true,
        );

        for candidate in candidates {
            let stored_locale = StoredLocale::new(candidate)
                .map_err(|_| RbacPresentationStoreError::InvalidStoredLocale)?;
            if let Some(presentation) = self
                .store
                .find_exact(tenant_id, resource_kind, resource_key, &stored_locale)
                .await?
            {
                return Ok(presentation.name);
            }
        }

        Err(RbacPresentationStoreError::NotFound)
    }
}

pub(crate) async fn seed_builtin_presentations_on<C>(
    connection: &C,
    tenant_id: Uuid,
) -> Result<(), RbacPresentationStoreError>
where
    C: ConnectionTrait,
{
    for (resource_key, name) in BUILTIN_ROLE_PRESENTATIONS {
        insert_seed_on(
            connection,
            tenant_id,
            RbacPresentationResourceKind::Role,
            resource_key,
            name,
        )
        .await?;
    }

    let catalog = BuiltinTenantRbacCatalog;
    for permission in catalog.permissions(tenant_id) {
        let name = builtin_permission_display_name(&permission.slug);
        insert_seed_on(
            connection,
            tenant_id,
            RbacPresentationResourceKind::Permission,
            &permission.slug,
            &name,
        )
        .await?;
    }

    Ok(())
}

fn builtin_permission_display_name(slug: &str) -> String {
    slug.replace(':', " / ")
}

async fn insert_seed_on<C>(
    connection: &C,
    tenant_id: Uuid,
    resource_kind: RbacPresentationResourceKind,
    resource_key: &str,
    name: &str,
) -> Result<(), RbacPresentationStoreError>
where
    C: ConnectionTrait,
{
    let backend = connection.get_database_backend();
    let sql = match backend {
        DbBackend::Postgres => {
            "INSERT INTO rbac_localized_presentations (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at) VALUES ($1, $2, $3, $4, $5, NULL, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) ON CONFLICT (tenant_id, resource_kind, resource_key, locale) DO NOTHING"
        }
        DbBackend::Sqlite => {
            "INSERT INTO rbac_localized_presentations (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, NULL, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP) ON CONFLICT (tenant_id, resource_kind, resource_key, locale) DO NOTHING"
        }
        DbBackend::MySql => {
            "INSERT IGNORE INTO rbac_localized_presentations (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at) VALUES (?, ?, ?, ?, ?, NULL, 1, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        }
        _ => {
            return Err(RbacPresentationStoreError::Storage(
                "unsupported database backend for RBAC presentation seed".to_string(),
            ));
        }
    };

    connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            sql,
            vec![
                tenant_id.into(),
                resource_kind.as_str().into(),
                resource_key.into(),
                PLATFORM_FALLBACK_LOCALE.into(),
                name.into(),
            ],
        ))
        .await
        .map_err(|error| RbacPresentationStoreError::Storage(error.to_string()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use sea_orm::Database;
    use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};

    use super::*;

    async fn database() -> DatabaseConnection {
        let database = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite RBAC presentation database should connect");
        let manager = SchemaManager::new(&database);
        crate::m20260914_000001_localized_presentations::Migration
            .up(&manager)
            .await
            .expect("RBAC presentation migration should apply");
        database
    }

    #[tokio::test]
    async fn localized_reader_uses_exact_language_then_platform_source_fallback() {
        let database = database().await;
        let tenant_id = Uuid::new_v4();
        seed_builtin_presentations_on(&database, tenant_id)
            .await
            .expect("built-in presentation should seed");

        let store = SeaOrmRbacPresentationStore::new(database.clone());
        store
            .create_source(
                tenant_id,
                RbacPresentationResourceKind::Role,
                "admin",
                RuntimeLocale::new("fr").expect("concrete French locale"),
                "Administrateur".to_string(),
                None,
            )
            .await
            .expect("localized admin copy should persist");

        let reader = RbacLocalizedCatalogReader::new(database);
        let roles = reader
            .roles(
                tenant_id,
                &RuntimeLocale::new("fr-CA").expect("concrete request locale"),
            )
            .await
            .expect("localized role catalog should resolve");

        assert_eq!(
            roles
                .iter()
                .find(|role| role.slug == "admin")
                .map(|role| role.display_name.as_str()),
            Some("Administrateur")
        );
        assert_eq!(
            roles
                .iter()
                .find(|role| role.slug == "manager")
                .map(|role| role.display_name.as_str()),
            Some("Manager")
        );
    }

    #[tokio::test]
    async fn presentation_edits_leave_authorization_identity_unchanged() {
        let database = database().await;
        let tenant_id = Uuid::new_v4();
        seed_builtin_presentations_on(&database, tenant_id)
            .await
            .expect("built-in presentation should seed");

        let identity_before = BuiltinTenantRbacCatalog
            .roles(tenant_id)
            .into_iter()
            .map(|role| (role.slug, role.permission_slugs))
            .collect::<Vec<_>>();

        let store = SeaOrmRbacPresentationStore::new(database.clone());
        store
            .compare_and_set_source(
                tenant_id,
                RbacPresentationResourceKind::Role,
                "admin",
                &RuntimeLocale::new(PLATFORM_FALLBACK_LOCALE)
                    .expect("platform fallback is concrete"),
                1,
                "Tenant Administrator".to_string(),
                Some("Presentation only".to_string()),
            )
            .await
            .expect("copy-only edit should succeed");

        let identity_after = RbacLocalizedCatalogReader::new(database)
            .roles(
                tenant_id,
                &RuntimeLocale::new(PLATFORM_FALLBACK_LOCALE)
                    .expect("platform fallback is concrete"),
            )
            .await
            .expect("localized role catalog should resolve")
            .into_iter()
            .map(|role| (role.slug, role.permission_slugs))
            .collect::<Vec<_>>();

        assert_eq!(identity_after, identity_before);
    }
}
