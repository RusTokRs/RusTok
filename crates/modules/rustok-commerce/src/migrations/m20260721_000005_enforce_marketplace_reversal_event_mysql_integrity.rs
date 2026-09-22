use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::MySql {
            return Ok(());
        }
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TRIGGER IF EXISTS marketplace_reversal_event_inbox_guard_insert;",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TRIGGER IF EXISTS marketplace_reversal_event_inbox_guard_update;",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER marketplace_reversal_event_inbox_guard_insert
                BEFORE INSERT ON marketplace_reversal_event_inbox
                FOR EACH ROW
                BEGIN
                    IF NEW.status NOT IN ('received', 'processing', 'retryable_error', 'operator_review', 'processed') THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'invalid marketplace reversal inbox status';
                    END IF;
                    IF TRIM(NEW.event_source) = ''
                        OR TRIM(NEW.event_id) = ''
                        OR NEW.event_hash NOT REGEXP '^[0-9a-f]{64}$'
                        OR NEW.reversal_kind NOT IN ('refund', 'chargeback')
                        OR NEW.currency_code NOT REGEXP '^[A-Z]{3}$'
                        OR NEW.currency_exponent < 0
                        OR NEW.currency_exponent > 9
                        OR NEW.total_amount <= 0
                        OR JSON_TYPE(NEW.lines_json) <> 'ARRAY'
                        OR JSON_LENGTH(NEW.lines_json) = 0
                        OR NEW.attempt_count < 0
                    THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'invalid marketplace reversal inbox identity';
                    END IF;
                    IF NOT (
                        (NEW.status = 'processing'
                            AND NEW.lease_owner IS NOT NULL
                            AND TRIM(NEW.lease_owner) <> ''
                            AND NEW.lease_expires_at IS NOT NULL)
                        OR
                        (NEW.status <> 'processing'
                            AND NEW.lease_owner IS NULL
                            AND NEW.lease_expires_at IS NULL)
                    ) THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'invalid marketplace reversal inbox lease';
                    END IF;
                    IF NOT (
                        (NEW.status = 'processed'
                            AND NEW.processed_at IS NOT NULL
                            AND NEW.reversal_id IS NOT NULL
                            AND NEW.ledger_transaction_id IS NOT NULL)
                        OR
                        (NEW.status <> 'processed'
                            AND NEW.processed_at IS NULL
                            AND NEW.reversal_id IS NULL
                            AND NEW.ledger_transaction_id IS NULL)
                    ) THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'invalid marketplace reversal inbox completion';
                    END IF;
                END
                "#,
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE TRIGGER marketplace_reversal_event_inbox_guard_update
                BEFORE UPDATE ON marketplace_reversal_event_inbox
                FOR EACH ROW
                BEGIN
                    IF NOT (NEW.id <=> OLD.id)
                        OR NOT (NEW.tenant_id <=> OLD.tenant_id)
                        OR NOT (NEW.provider_event_id <=> OLD.provider_event_id)
                        OR NOT (NEW.event_source <=> OLD.event_source)
                        OR NOT (NEW.event_id <=> OLD.event_id)
                        OR NOT (NEW.event_hash <=> OLD.event_hash)
                        OR NOT (NEW.reversal_kind <=> OLD.reversal_kind)
                        OR NOT (NEW.source_id <=> OLD.source_id)
                        OR NOT (NEW.order_id <=> OLD.order_id)
                        OR NOT (NEW.payment_collection_id <=> OLD.payment_collection_id)
                        OR NOT (NEW.occurred_at <=> OLD.occurred_at)
                        OR NOT (NEW.currency_code <=> OLD.currency_code)
                        OR NOT (NEW.currency_exponent <=> OLD.currency_exponent)
                        OR NOT (NEW.total_amount <=> OLD.total_amount)
                        OR NOT (NEW.lines_json <=> OLD.lines_json)
                    THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'marketplace reversal normalized facts are immutable';
                    END IF;
                    IF OLD.status = 'processed' THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'processed marketplace reversal inbox row is immutable';
                    END IF;
                    IF NEW.status NOT IN ('received', 'processing', 'retryable_error', 'operator_review', 'processed') THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'invalid marketplace reversal inbox status';
                    END IF;
                    IF NOT (
                        (NEW.status = 'processing'
                            AND NEW.lease_owner IS NOT NULL
                            AND TRIM(NEW.lease_owner) <> ''
                            AND NEW.lease_expires_at IS NOT NULL)
                        OR
                        (NEW.status <> 'processing'
                            AND NEW.lease_owner IS NULL
                            AND NEW.lease_expires_at IS NULL)
                    ) THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'invalid marketplace reversal inbox lease';
                    END IF;
                    IF NOT (
                        (NEW.status = 'processed'
                            AND NEW.processed_at IS NOT NULL
                            AND NEW.reversal_id IS NOT NULL
                            AND NEW.ledger_transaction_id IS NOT NULL)
                        OR
                        (NEW.status <> 'processed'
                            AND NEW.processed_at IS NULL
                            AND NEW.reversal_id IS NULL
                            AND NEW.ledger_transaction_id IS NULL)
                    ) THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'invalid marketplace reversal inbox completion';
                    END IF;
                END
                "#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::MySql {
            return Ok(());
        }
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TRIGGER IF EXISTS marketplace_reversal_event_inbox_guard_update;",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "DROP TRIGGER IF EXISTS marketplace_reversal_event_inbox_guard_insert;",
            )
            .await?;
        Ok(())
    }
}
