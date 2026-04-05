use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE tenants
    ALTER COLUMN default_locale TYPE VARCHAR(32);

ALTER TABLE tenant_locales
    ALTER COLUMN locale TYPE VARCHAR(32),
    ALTER COLUMN fallback_locale TYPE VARCHAR(32);
"#,
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE tenant_locales
    ALTER COLUMN fallback_locale TYPE VARCHAR(5),
    ALTER COLUMN locale TYPE VARCHAR(5);

ALTER TABLE tenants
    ALTER COLUMN default_locale TYPE VARCHAR(5);
"#,
            )
            .await
    }
}
