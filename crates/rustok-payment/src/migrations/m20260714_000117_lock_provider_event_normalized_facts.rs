use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

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
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_provider_events_normalized_guard
                            ON payment_provider_events;
                        DROP FUNCTION IF EXISTS enforce_payment_provider_event_normalized_immutability();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite | DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS payment_provider_events_normalized_guard;",
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION enforce_payment_provider_event_normalized_immutability()
            RETURNS trigger AS $$
            BEGIN
                IF (OLD.event_type IS NOT NULL OR OLD.event_metadata IS NOT NULL)
                    AND (
                        NEW.event_type IS DISTINCT FROM OLD.event_type
                        OR NEW.external_reference IS DISTINCT FROM OLD.external_reference
                        OR NEW.event_metadata IS DISTINCT FROM OLD.event_metadata
                    )
                THEN
                    RAISE EXCEPTION 'payment provider event normalized facts are immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER payment_provider_events_normalized_guard
            BEFORE UPDATE ON payment_provider_events
            FOR EACH ROW
            EXECUTE FUNCTION enforce_payment_provider_event_normalized_immutability();
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
            CREATE TRIGGER payment_provider_events_normalized_guard
            BEFORE UPDATE ON payment_provider_events
            FOR EACH ROW
            WHEN (OLD.event_type IS NOT NULL OR OLD.event_metadata IS NOT NULL)
             AND (
                NEW.event_type IS NOT OLD.event_type
                OR NEW.external_reference IS NOT OLD.external_reference
                OR NEW.event_metadata IS NOT OLD.event_metadata
             )
            BEGIN
                SELECT RAISE(ABORT, 'payment provider event normalized facts are immutable');
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
            CREATE TRIGGER payment_provider_events_normalized_guard
            BEFORE UPDATE ON payment_provider_events
            FOR EACH ROW
            BEGIN
                IF (OLD.event_type IS NOT NULL OR OLD.event_metadata IS NOT NULL)
                    AND (
                        NOT (NEW.event_type <=> OLD.event_type)
                        OR NOT (NEW.external_reference <=> OLD.external_reference)
                        OR NOT (
                            CAST(NEW.event_metadata AS CHAR)
                            <=> CAST(OLD.event_metadata AS CHAR)
                        )
                    )
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'payment provider event normalized facts are immutable';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}
