use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Refunds::Table)
                    .add_column(ColumnDef::new(Refunds::CreationKey).string_len(191))
                    .add_column(ColumnDef::new(Refunds::CreationRequestHash).string_len(64))
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ux_refunds_creation_identity")
                    .table(Refunds::Table)
                    .col(Refunds::TenantId)
                    .col(Refunds::PaymentCollectionId)
                    .col(Refunds::CreationKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
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
                        DROP TRIGGER IF EXISTS refunds_creation_identity_guard ON refunds;
                        DROP FUNCTION IF EXISTS enforce_refund_creation_identity();
                        ALTER TABLE refunds
                            DROP CONSTRAINT IF EXISTS ck_refunds_creation_identity;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS refunds_creation_identity_guard_insert;
                        DROP TRIGGER IF EXISTS refunds_creation_identity_guard_update;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS refunds_creation_identity_guard_insert;
                        DROP TRIGGER IF EXISTS refunds_creation_identity_guard_update;
                        "#,
                    )
                    .await?;
            }
        }
        manager
            .drop_index(
                Index::drop()
                    .name("ux_refunds_creation_identity")
                    .table(Refunds::Table)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(Refunds::Table)
                    .drop_column(Refunds::CreationRequestHash)
                    .drop_column(Refunds::CreationKey)
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE refunds
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

            CREATE OR REPLACE FUNCTION enforce_refund_creation_identity()
            RETURNS trigger AS $$
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.creation_key IS DISTINCT FROM OLD.creation_key
                    OR NEW.creation_request_hash IS DISTINCT FROM OLD.creation_request_hash
                ) THEN
                    RAISE EXCEPTION 'refund creation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER refunds_creation_identity_guard
            BEFORE UPDATE ON refunds
            FOR EACH ROW
            EXECUTE FUNCTION enforce_refund_creation_identity();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
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

            CREATE TRIGGER refunds_creation_identity_guard_update
            BEFORE UPDATE ON refunds
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.creation_key IS NOT OLD.creation_key
                    OR NEW.creation_request_hash IS NOT OLD.creation_request_hash
                    THEN RAISE(ABORT, 'refund creation identity is immutable') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
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

            CREATE TRIGGER refunds_creation_identity_guard_update
            BEFORE UPDATE ON refunds
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.creation_key <=> OLD.creation_key)
                    OR NOT (NEW.creation_request_hash <=> OLD.creation_request_hash)
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'refund creation identity is immutable';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum Refunds {
    Table,
    TenantId,
    PaymentCollectionId,
    CreationKey,
    CreationRequestHash,
}
