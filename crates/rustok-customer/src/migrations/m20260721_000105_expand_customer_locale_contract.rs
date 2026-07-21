use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE customers ALTER COLUMN locale TYPE VARCHAR(32)",
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE customers MODIFY COLUMN locale VARCHAR(32) NULL",
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => rebuild_sqlite_customers(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Forward-only: narrowing the locale column risks truncating valid normalized tags.
        Ok(())
    }
}

async fn rebuild_sqlite_customers(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
DROP TRIGGER IF EXISTS customers_identity_guard_update;
DROP TRIGGER IF EXISTS customers_identity_guard_insert;
DROP INDEX IF EXISTS ux_customers_tenant_email_canonical;

ALTER TABLE customers RENAME TO customers_locale_v16;
CREATE TABLE customers (
    id TEXT NOT NULL PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    user_id TEXT NULL,
    email VARCHAR(255) NOT NULL,
    first_name VARCHAR(100) NULL,
    last_name VARCHAR(100) NULL,
    phone VARCHAR(50) NULL,
    locale VARCHAR(32) NULL,
    metadata JSON NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO customers (
    id, tenant_id, user_id, email, first_name, last_name, phone, locale, metadata,
    created_at, updated_at
)
SELECT
    id, tenant_id, user_id, email, first_name, last_name, phone, locale, metadata,
    created_at, updated_at
FROM customers_locale_v16;
DROP TABLE customers_locale_v16;

CREATE UNIQUE INDEX idx_customers_tenant_email ON customers (tenant_id, email);
CREATE UNIQUE INDEX idx_customers_tenant_user ON customers (tenant_id, user_id);
CREATE UNIQUE INDEX ux_customers_tenant_email_canonical
    ON customers (tenant_id, email COLLATE NOCASE);

CREATE TRIGGER customers_identity_guard_insert
BEFORE INSERT ON customers FOR EACH ROW BEGIN
    SELECT CASE WHEN trim(NEW.email) = '' OR NEW.email <> trim(NEW.email)
        THEN RAISE(ABORT, 'invalid customer email') END;
    SELECT CASE WHEN NEW.locale IS NOT NULL AND (
        length(trim(NEW.locale)) < 2 OR length(trim(NEW.locale)) > 32
        OR trim(NEW.locale) GLOB '*[^A-Za-z0-9_-]*'
    ) THEN RAISE(ABORT, 'invalid customer locale') END;
END;

CREATE TRIGGER customers_identity_guard_update
BEFORE UPDATE OF email, locale ON customers FOR EACH ROW BEGIN
    SELECT CASE WHEN trim(NEW.email) = '' OR NEW.email <> trim(NEW.email)
        THEN RAISE(ABORT, 'invalid customer email') END;
    SELECT CASE WHEN NEW.locale IS NOT NULL AND (
        length(trim(NEW.locale)) < 2 OR length(trim(NEW.locale)) > 32
        OR trim(NEW.locale) GLOB '*[^A-Za-z0-9_-]*'
    ) THEN RAISE(ABORT, 'invalid customer locale') END;
END;
"#,
        )
        .await?;
    Ok(())
}
