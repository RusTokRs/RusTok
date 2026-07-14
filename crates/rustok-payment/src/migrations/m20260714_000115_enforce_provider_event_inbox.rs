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
                        DROP TRIGGER IF EXISTS payment_provider_events_integrity_guard
                            ON payment_provider_events;
                        DROP FUNCTION IF EXISTS enforce_payment_provider_event_integrity();
                        ALTER TABLE payment_provider_events
                            DROP CONSTRAINT IF EXISTS ck_payment_provider_events_identity,
                            DROP CONSTRAINT IF EXISTS ck_payment_provider_events_state;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_provider_events_guard_insert;
                        DROP TRIGGER IF EXISTS payment_provider_events_guard_update;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_provider_events_guard_insert;
                        DROP TRIGGER IF EXISTS payment_provider_events_guard_update;
                        "#,
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
            ALTER TABLE payment_provider_events
                ADD CONSTRAINT ck_payment_provider_events_identity
                CHECK (
                    btrim(provider_id) <> ''
                    AND btrim(delivery_id) <> ''
                    AND btrim(idempotency_key) <> ''
                    AND payload_hash ~ '^[0-9a-f]{64}$'
                    AND signature_verified
                    AND attempt_count >= 0
                ),
                ADD CONSTRAINT ck_payment_provider_events_state
                CHECK (
                    (status = 'received'
                        AND lease_owner IS NULL
                        AND lease_expires_at IS NULL
                        AND error_code IS NULL
                        AND error_message IS NULL
                        AND processed_at IS NULL)
                    OR
                    (status = 'processing'
                        AND lease_owner IS NOT NULL
                        AND btrim(lease_owner) <> ''
                        AND lease_expires_at IS NOT NULL
                        AND error_code IS NULL
                        AND error_message IS NULL
                        AND processed_at IS NULL)
                    OR
                    (status = 'failed'
                        AND lease_owner IS NULL
                        AND lease_expires_at IS NULL
                        AND error_code IS NOT NULL
                        AND btrim(error_code) <> ''
                        AND error_message IS NOT NULL
                        AND btrim(error_message) <> ''
                        AND processed_at IS NULL)
                    OR
                    (status = 'processed'
                        AND lease_owner IS NULL
                        AND lease_expires_at IS NULL
                        AND event_type IS NOT NULL
                        AND btrim(event_type) <> ''
                        AND error_code IS NULL
                        AND error_message IS NULL
                        AND processed_at IS NOT NULL)
                    OR
                    (status = 'dead_letter'
                        AND lease_owner IS NULL
                        AND lease_expires_at IS NULL
                        AND error_code IS NOT NULL
                        AND btrim(error_code) <> ''
                        AND error_message IS NOT NULL
                        AND btrim(error_message) <> ''
                        AND processed_at IS NOT NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_payment_provider_event_integrity()
            RETURNS trigger AS $$
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
                    OR NEW.delivery_id IS DISTINCT FROM OLD.delivery_id
                    OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                    OR NEW.payload_hash IS DISTINCT FROM OLD.payload_hash
                    OR NEW.signature_verified IS DISTINCT FROM OLD.signature_verified
                    OR NEW.received_at IS DISTINCT FROM OLD.received_at
                ) THEN
                    RAISE EXCEPTION 'payment provider event identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF TG_OP = 'UPDATE' AND NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('received', 'failed') AND NEW.status = 'processing')
                    OR (OLD.status = 'processing' AND NEW.status IN (
                        'processed', 'failed', 'dead_letter'
                    ))
                    OR (OLD.status = 'failed' AND NEW.status = 'dead_letter')
                ) THEN
                    RAISE EXCEPTION 'invalid payment provider event transition from % to %',
                        OLD.status,
                        NEW.status
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER payment_provider_events_integrity_guard
            BEFORE INSERT OR UPDATE ON payment_provider_events
            FOR EACH ROW
            EXECUTE FUNCTION enforce_payment_provider_event_integrity();
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
            CREATE TRIGGER payment_provider_events_guard_insert
            BEFORE INSERT ON payment_provider_events
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN trim(NEW.provider_id) = ''
                    OR trim(NEW.delivery_id) = ''
                    OR trim(NEW.idempotency_key) = ''
                    OR length(NEW.payload_hash) <> 64
                    OR NEW.payload_hash GLOB '*[^0-9a-f]*'
                    OR NEW.signature_verified <> 1
                    OR NEW.attempt_count < 0
                    THEN RAISE(ABORT, 'invalid payment provider event identity') END;
                SELECT CASE WHEN NOT (
                    NEW.status = 'received'
                    AND NEW.lease_owner IS NULL
                    AND NEW.lease_expires_at IS NULL
                    AND NEW.error_code IS NULL
                    AND NEW.error_message IS NULL
                    AND NEW.processed_at IS NULL
                ) THEN RAISE(ABORT, 'invalid received payment provider event') END;
            END;

            CREATE TRIGGER payment_provider_events_guard_update
            BEFORE UPDATE ON payment_provider_events
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.provider_id IS NOT OLD.provider_id
                    OR NEW.delivery_id IS NOT OLD.delivery_id
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.payload_hash IS NOT OLD.payload_hash
                    OR NEW.signature_verified IS NOT OLD.signature_verified
                    OR NEW.received_at IS NOT OLD.received_at
                    THEN RAISE(ABORT, 'payment provider event identity is immutable') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('received', 'failed') AND NEW.status = 'processing')
                    OR (OLD.status = 'processing' AND NEW.status IN (
                        'processed', 'failed', 'dead_letter'
                    ))
                    OR (OLD.status = 'failed' AND NEW.status = 'dead_letter')
                ) THEN RAISE(ABORT, 'invalid payment provider event transition') END;
                SELECT CASE WHEN NEW.attempt_count < 0
                    OR NEW.signature_verified <> 1
                    THEN RAISE(ABORT, 'invalid payment provider event identity') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'received'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL
                        AND NEW.error_code IS NULL
                        AND NEW.error_message IS NULL
                        AND NEW.processed_at IS NULL)
                    OR
                    (NEW.status = 'processing'
                        AND NEW.lease_owner IS NOT NULL
                        AND trim(NEW.lease_owner) <> ''
                        AND NEW.lease_expires_at IS NOT NULL
                        AND NEW.error_code IS NULL
                        AND NEW.error_message IS NULL
                        AND NEW.processed_at IS NULL)
                    OR
                    (NEW.status = 'failed'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL
                        AND NEW.error_code IS NOT NULL
                        AND trim(NEW.error_code) <> ''
                        AND NEW.error_message IS NOT NULL
                        AND trim(NEW.error_message) <> ''
                        AND NEW.processed_at IS NULL)
                    OR
                    (NEW.status = 'processed'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL
                        AND NEW.event_type IS NOT NULL
                        AND trim(NEW.event_type) <> ''
                        AND NEW.error_code IS NULL
                        AND NEW.error_message IS NULL
                        AND NEW.processed_at IS NOT NULL)
                    OR
                    (NEW.status = 'dead_letter'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL
                        AND NEW.error_code IS NOT NULL
                        AND trim(NEW.error_code) <> ''
                        AND NEW.error_message IS NOT NULL
                        AND trim(NEW.error_message) <> ''
                        AND NEW.processed_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid payment provider event state') END;
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
            CREATE TRIGGER payment_provider_events_guard_insert
            BEFORE INSERT ON payment_provider_events
            FOR EACH ROW
            BEGIN
                IF TRIM(NEW.provider_id) = ''
                    OR TRIM(NEW.delivery_id) = ''
                    OR TRIM(NEW.idempotency_key) = ''
                    OR CHAR_LENGTH(NEW.payload_hash) <> 64
                    OR NEW.payload_hash REGEXP '[^0-9a-f]'
                    OR NEW.signature_verified <> 1
                    OR NEW.attempt_count < 0
                    OR NEW.status <> 'received'
                    OR NEW.lease_owner IS NOT NULL
                    OR NEW.lease_expires_at IS NOT NULL
                    OR NEW.error_code IS NOT NULL
                    OR NEW.error_message IS NOT NULL
                    OR NEW.processed_at IS NOT NULL
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid payment provider event insert';
                END IF;
            END;

            CREATE TRIGGER payment_provider_events_guard_update
            BEFORE UPDATE ON payment_provider_events
            FOR EACH ROW
            BEGIN
                IF NEW.id <> OLD.id
                    OR NEW.tenant_id <> OLD.tenant_id
                    OR NEW.provider_id <> OLD.provider_id
                    OR NEW.delivery_id <> OLD.delivery_id
                    OR NEW.idempotency_key <> OLD.idempotency_key
                    OR NEW.payload_hash <> OLD.payload_hash
                    OR NEW.signature_verified <> OLD.signature_verified
                    OR NEW.received_at <> OLD.received_at
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'payment provider event identity is immutable';
                END IF;
                IF NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('received', 'failed') AND NEW.status = 'processing')
                    OR (OLD.status = 'processing' AND NEW.status IN (
                        'processed', 'failed', 'dead_letter'
                    ))
                    OR (OLD.status = 'failed' AND NEW.status = 'dead_letter')
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid payment provider event transition';
                END IF;
                IF NOT (
                    (NEW.status = 'received'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL
                        AND NEW.error_code IS NULL
                        AND NEW.error_message IS NULL
                        AND NEW.processed_at IS NULL)
                    OR
                    (NEW.status = 'processing'
                        AND NEW.lease_owner IS NOT NULL
                        AND TRIM(NEW.lease_owner) <> ''
                        AND NEW.lease_expires_at IS NOT NULL
                        AND NEW.error_code IS NULL
                        AND NEW.error_message IS NULL
                        AND NEW.processed_at IS NULL)
                    OR
                    (NEW.status = 'failed'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL
                        AND NEW.error_code IS NOT NULL
                        AND TRIM(NEW.error_code) <> ''
                        AND NEW.error_message IS NOT NULL
                        AND TRIM(NEW.error_message) <> ''
                        AND NEW.processed_at IS NULL)
                    OR
                    (NEW.status = 'processed'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL
                        AND NEW.event_type IS NOT NULL
                        AND TRIM(NEW.event_type) <> ''
                        AND NEW.error_code IS NULL
                        AND NEW.error_message IS NULL
                        AND NEW.processed_at IS NOT NULL)
                    OR
                    (NEW.status = 'dead_letter'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL
                        AND NEW.error_code IS NOT NULL
                        AND TRIM(NEW.error_code) <> ''
                        AND NEW.error_message IS NOT NULL
                        AND TRIM(NEW.error_message) <> ''
                        AND NEW.processed_at IS NOT NULL)
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid payment provider event state';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}
