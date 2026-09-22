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
                        r#"
                        CREATE UNIQUE INDEX ux_customers_tenant_email_canonical
                        ON customers (tenant_id, lower(btrim(email)));

                        ALTER TABLE customers
                            ADD CONSTRAINT ck_customers_email
                            CHECK (btrim(email) <> '' AND email = btrim(email)) NOT VALID,
                            ADD CONSTRAINT ck_customers_locale
                            CHECK (
                                locale IS NULL OR (
                                    octet_length(locale) <= 32
                                    AND locale ~ '^[A-Za-z]{2,8}([_-][A-Za-z0-9]{1,8})*$'
                                )
                            ) NOT VALID;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
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
            }
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE customers
                            DROP CONSTRAINT IF EXISTS ck_customers_locale,
                            DROP CONSTRAINT IF EXISTS ck_customers_email;
                        DROP INDEX IF EXISTS ux_customers_tenant_email_canonical;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS customers_identity_guard_update;
                        DROP TRIGGER IF EXISTS customers_identity_guard_insert;
                        DROP INDEX IF EXISTS ux_customers_tenant_email_canonical;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}
