use sea_orm::{ConnectionTrait, DbBackend};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(RbacLocalizedPresentations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::ResourceKind)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::ResourceKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::Name)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(RbacLocalizedPresentations::Description).text())
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::CopyRevision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(RbacLocalizedPresentations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .col(RbacLocalizedPresentations::TenantId)
                            .col(RbacLocalizedPresentations::ResourceKind)
                            .col(RbacLocalizedPresentations::ResourceKey)
                            .col(RbacLocalizedPresentations::Locale),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_rbac_localized_presentations_resource")
                    .table(RbacLocalizedPresentations::Table)
                    .col(RbacLocalizedPresentations::TenantId)
                    .col(RbacLocalizedPresentations::ResourceKind)
                    .col(RbacLocalizedPresentations::ResourceKey)
                    .to_owned(),
            )
            .await?;

        seed_builtin_presentations(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(RbacLocalizedPresentations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn seed_builtin_presentations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let connection = manager.get_connection();
    let (role_sql, permission_sql) = match connection.get_database_backend() {
        DbBackend::Postgres | DbBackend::Sqlite => (
            r#"
INSERT INTO rbac_localized_presentations
    (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at)
SELECT
    r.tenant_id,
    'role',
    r.slug,
    'en',
    CASE r.slug
        WHEN 'super_admin' THEN 'Super Admin'
        WHEN 'admin' THEN 'Admin'
        WHEN 'manager' THEN 'Manager'
        WHEN 'customer' THEN 'Customer'
    END,
    NULL,
    1,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
FROM roles r
WHERE r.is_system = TRUE
  AND r.slug IN ('super_admin', 'admin', 'manager', 'customer')
ON CONFLICT (tenant_id, resource_kind, resource_key, locale) DO NOTHING
"#,
            r#"
INSERT INTO rbac_localized_presentations
    (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at)
SELECT DISTINCT
    p.tenant_id,
    'permission',
    p.resource || ':' || p.action,
    'en',
    REPLACE(p.resource || ':' || p.action, ':', ' / '),
    NULL,
    1,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
FROM permissions p
JOIN role_permissions rp ON rp.permission_id = p.id
JOIN roles r ON r.id = rp.role_id AND r.tenant_id = p.tenant_id
WHERE r.is_system = TRUE
  AND r.slug IN ('super_admin', 'admin', 'manager', 'customer')
ON CONFLICT (tenant_id, resource_kind, resource_key, locale) DO NOTHING
"#,
        ),
        DbBackend::MySql => (
            r#"
INSERT IGNORE INTO rbac_localized_presentations
    (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at)
SELECT
    r.tenant_id,
    'role',
    r.slug,
    'en',
    CASE r.slug
        WHEN 'super_admin' THEN 'Super Admin'
        WHEN 'admin' THEN 'Admin'
        WHEN 'manager' THEN 'Manager'
        WHEN 'customer' THEN 'Customer'
    END,
    NULL,
    1,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
FROM roles r
WHERE r.is_system = TRUE
  AND r.slug IN ('super_admin', 'admin', 'manager', 'customer')
"#,
            r#"
INSERT IGNORE INTO rbac_localized_presentations
    (tenant_id, resource_kind, resource_key, locale, name, description, copy_revision, created_at, updated_at)
SELECT DISTINCT
    p.tenant_id,
    'permission',
    CONCAT(p.resource, ':', p.action),
    'en',
    REPLACE(CONCAT(p.resource, ':', p.action), ':', ' / '),
    NULL,
    1,
    CURRENT_TIMESTAMP,
    CURRENT_TIMESTAMP
FROM permissions p
JOIN role_permissions rp ON rp.permission_id = p.id
JOIN roles r ON r.id = rp.role_id AND r.tenant_id = p.tenant_id
WHERE r.is_system = TRUE
  AND r.slug IN ('super_admin', 'admin', 'manager', 'customer')
"#,
        ),
        _ => {
            return Err(DbErr::Custom(
                "RBAC localized presentation seed does not support this database backend"
                    .to_string(),
            ));
        }
    };

    connection.execute_unprepared(role_sql).await?;
    connection.execute_unprepared(permission_sql).await?;
    Ok(())
}

#[derive(Iden)]
enum RbacLocalizedPresentations {
    #[iden = "rbac_localized_presentations"]
    Table,
    TenantId,
    ResourceKind,
    ResourceKey,
    Locale,
    Name,
    Description,
    CopyRevision,
    CreatedAt,
    UpdatedAt,
}