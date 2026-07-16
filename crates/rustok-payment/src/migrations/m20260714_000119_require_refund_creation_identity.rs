use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const LEGACY_REQUEST_HASH: &str =
    "0000000000000000000000000000000000000000000000000000000000000000";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => uninstall_postgres(manager).await?,
            DatabaseBackend::Sqlite => uninstall_sqlite(manager).await?,
            DatabaseBackend::MySql => uninstall_mysql(manager).await?,
        }
        Ok(())
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            UPDATE refunds
            SET creation_key = COALESCE(creation_key, 'legacy:' || id::text),
                creation_request_hash = COALESCE(creation_request_hash, '{LEGACY_REQUEST_HASH}')
            WHERE creation_key IS NULL OR creation_request_hash IS NULL;

            ALTER TABLE refunds
                DROP CONSTRAINT IF EXISTS ck_refunds_creation_identity;
            ALTER TABLE refunds
                ALTER COLUMN creation_key SET NOT NULL,
                ALTER COLUMN creation_request_hash SET NOT NULL,
                ADD CONSTRAINT ck_refunds_creation_identity
                CHECK (
                    btrim(creation_key) <> ''
                    AND creation_request_hash ~ '^[0-9a-f]{{64}}$'
                );
            "#
        ))
        .await?;
    Ok(())
}

async fn uninstall_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE refunds
                DROP CONSTRAINT IF EXISTS ck_refunds_creation_identity;
            ALTER TABLE refunds
                ALTER COLUMN creation_key DROP NOT NULL,
                ALTER COLUMN creation_request_hash DROP NOT NULL,
                ADD CONSTRAINT ck_refunds_creation_identity
                CHECK (
                    (creation_key IS NULL AND creation_request_hash IS NULL)
                    OR
                    (
                        creation_key IS NOT NULL
                        AND btrim(creation_key) <> ''
                        AND creation_request_hash ~ '^[0-9a-f]{64}$'
                    )
                );
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            UPDATE refunds
            SET creation_key = COALESCE(creation_key, 'legacy:' || id),
                creation_request_hash = COALESCE(creation_request_hash, '{LEGACY_REQUEST_HASH}')
            WHERE creation_key IS NULL OR creation_request_hash IS NULL;

            DROP TRIGGER IF EXISTS refunds_creation_identity_guard_insert;
            CREATE TRIGGER refunds_creation_identity_guard_insert
            BEFORE INSERT ON refunds
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.creation_key IS NULL
                    OR trim(NEW.creation_key) = ''
                    OR NEW.creation_request_hash IS NULL
                    OR length(NEW.creation_request_hash) <> 64
                    OR NEW.creation_request_hash GLOB '*[^0-9a-f]*'
                    THEN RAISE(ABORT, 'refund creation identity is required') END;
            END;
            "#
        ))
        .await?;
    Ok(())
}

async fn uninstall_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS refunds_creation_identity_guard_insert;
            CREATE TRIGGER refunds_creation_identity_guard_insert
            BEFORE INSERT ON refunds
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT (
                    (NEW.creation_key IS NULL AND NEW.creation_request_hash IS NULL)
                    OR
                    (
                        NEW.creation_key IS NOT NULL
                        AND trim(NEW.creation_key) <> ''
                        AND NEW.creation_request_hash IS NOT NULL
                        AND length(NEW.creation_request_hash) = 64
                        AND NEW.creation_request_hash NOT GLOB '*[^0-9a-f]*'
                    )
                ) THEN RAISE(ABORT, 'invalid refund creation identity') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            UPDATE refunds
            SET creation_key = COALESCE(creation_key, CONCAT('legacy:', id)),
                creation_request_hash = COALESCE(creation_request_hash, '{LEGACY_REQUEST_HASH}')
            WHERE creation_key IS NULL OR creation_request_hash IS NULL;

            DROP TRIGGER IF EXISTS refunds_creation_identity_guard_insert;
            ALTER TABLE refunds
                MODIFY creation_key VARCHAR(191) NOT NULL,
                MODIFY creation_request_hash VARCHAR(64) NOT NULL;

            CREATE TRIGGER refunds_creation_identity_guard_insert
            BEFORE INSERT ON refunds
            FOR EACH ROW
            BEGIN
                IF TRIM(NEW.creation_key) = ''
                    OR NEW.creation_request_hash NOT REGEXP '^[0-9a-f]{{64}}$'
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'refund creation identity is required';
                END IF;
            END;
            "#
        ))
        .await?;
    Ok(())
}

async fn uninstall_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS refunds_creation_identity_guard_insert;
            ALTER TABLE refunds
                MODIFY creation_key VARCHAR(191) NULL,
                MODIFY creation_request_hash VARCHAR(64) NULL;

            CREATE TRIGGER refunds_creation_identity_guard_insert
            BEFORE INSERT ON refunds
            FOR EACH ROW
            BEGIN
                IF NOT (
                    (NEW.creation_key IS NULL AND NEW.creation_request_hash IS NULL)
                    OR
                    (
                        NEW.creation_key IS NOT NULL
                        AND TRIM(NEW.creation_key) <> ''
                        AND NEW.creation_request_hash REGEXP '^[0-9a-f]{64}$'
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid refund creation identity';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}
