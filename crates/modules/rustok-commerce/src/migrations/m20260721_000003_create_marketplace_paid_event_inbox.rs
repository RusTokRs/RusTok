use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MarketplacePaidEventInbox::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::EventSource)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::EventId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::EventHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::CheckoutOperationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::OrderId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::PaymentCollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::CapturedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::CapturedAmount)
                            .decimal()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(MarketplacePaidEventInbox::LeaseOwner).string_len(191))
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(MarketplacePaidEventInbox::LastErrorCode).string_len(100))
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::LastErrorMessage)
                            .string_len(2000),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplacePaidEventInbox::ProcessedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                MarketplacePaidEventInbox::Table,
                                MarketplacePaidEventInbox::CheckoutOperationId,
                            )
                            .to(CheckoutOperations::Table, CheckoutOperations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ux_marketplace_paid_event_inbox_source_event")
                    .table(MarketplacePaidEventInbox::Table)
                    .col(MarketplacePaidEventInbox::TenantId)
                    .col(MarketplacePaidEventInbox::EventSource)
                    .col(MarketplacePaidEventInbox::EventId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_marketplace_paid_event_inbox_checkout")
                    .table(MarketplacePaidEventInbox::Table)
                    .col(MarketplacePaidEventInbox::TenantId)
                    .col(MarketplacePaidEventInbox::CheckoutOperationId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_marketplace_paid_event_inbox_recovery")
                    .table(MarketplacePaidEventInbox::Table)
                    .col(MarketplacePaidEventInbox::Status)
                    .col(MarketplacePaidEventInbox::LeaseExpiresAt)
                    .col(MarketplacePaidEventInbox::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplacePaidEventInbox::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE marketplace_paid_event_inbox
                ADD CONSTRAINT ck_marketplace_paid_event_inbox_status
                CHECK (status IN ('received', 'processing', 'retryable_error', 'operator_review', 'processed')),
                ADD CONSTRAINT ck_marketplace_paid_event_inbox_identity
                CHECK (
                    btrim(event_source) <> ''
                    AND btrim(event_id) <> ''
                    AND event_hash ~ '^[0-9a-f]{64}$'
                    AND currency_code ~ '^[A-Z]{3}$'
                    AND captured_amount >= 0
                    AND attempt_count >= 0
                ),
                ADD CONSTRAINT ck_marketplace_paid_event_inbox_lease
                CHECK (
                    (status = 'processing' AND lease_owner IS NOT NULL AND btrim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
                    OR
                    (status <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
                ),
                ADD CONSTRAINT ck_marketplace_paid_event_inbox_completion
                CHECK (
                    (status = 'processed' AND processed_at IS NOT NULL)
                    OR
                    (status <> 'processed' AND processed_at IS NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_marketplace_paid_event_inbox_integrity()
            RETURNS trigger AS $$
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.event_source IS DISTINCT FROM OLD.event_source
                    OR NEW.event_id IS DISTINCT FROM OLD.event_id
                    OR NEW.event_hash IS DISTINCT FROM OLD.event_hash
                    OR NEW.checkout_operation_id IS DISTINCT FROM OLD.checkout_operation_id
                    OR NEW.order_id IS DISTINCT FROM OLD.order_id
                    OR NEW.payment_collection_id IS DISTINCT FROM OLD.payment_collection_id
                    OR NEW.captured_at IS DISTINCT FROM OLD.captured_at
                    OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
                    OR NEW.captured_amount IS DISTINCT FROM OLD.captured_amount
                ) THEN
                    RAISE EXCEPTION 'marketplace paid-event normalized facts are immutable'
                        USING ERRCODE = '23514';
                END IF;
                IF TG_OP = 'UPDATE' AND OLD.status = 'processed' AND NEW IS DISTINCT FROM OLD THEN
                    RAISE EXCEPTION 'processed marketplace paid-event inbox row is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER marketplace_paid_event_inbox_integrity_guard
            BEFORE UPDATE ON marketplace_paid_event_inbox
            FOR EACH ROW EXECUTE FUNCTION enforce_marketplace_paid_event_inbox_integrity();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER marketplace_paid_event_inbox_guard_insert
            BEFORE INSERT ON marketplace_paid_event_inbox
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.status NOT IN ('received', 'processing', 'retryable_error', 'operator_review', 'processed')
                    THEN RAISE(ABORT, 'invalid marketplace paid-event inbox status') END;
                SELECT CASE WHEN trim(NEW.event_source) = '' OR trim(NEW.event_id) = ''
                    OR length(NEW.event_hash) <> 64 OR length(NEW.currency_code) <> 3
                    OR NEW.currency_code <> upper(NEW.currency_code)
                    OR NEW.captured_amount < 0 OR NEW.attempt_count < 0
                    THEN RAISE(ABORT, 'invalid marketplace paid-event inbox identity') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'processing' AND NEW.lease_owner IS NOT NULL AND trim(NEW.lease_owner) <> '' AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status <> 'processing' AND NEW.lease_owner IS NULL AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace paid-event inbox lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'processed' AND NEW.processed_at IS NOT NULL)
                    OR
                    (NEW.status <> 'processed' AND NEW.processed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace paid-event inbox completion') END;
            END;

            CREATE TRIGGER marketplace_paid_event_inbox_guard_update
            BEFORE UPDATE ON marketplace_paid_event_inbox
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.event_source IS NOT OLD.event_source
                    OR NEW.event_id IS NOT OLD.event_id
                    OR NEW.event_hash IS NOT OLD.event_hash
                    OR NEW.checkout_operation_id IS NOT OLD.checkout_operation_id
                    OR NEW.order_id IS NOT OLD.order_id
                    OR NEW.payment_collection_id IS NOT OLD.payment_collection_id
                    OR NEW.captured_at IS NOT OLD.captured_at
                    OR NEW.currency_code IS NOT OLD.currency_code
                    OR NEW.captured_amount IS NOT OLD.captured_amount
                    THEN RAISE(ABORT, 'marketplace paid-event normalized facts are immutable') END;
                SELECT CASE WHEN OLD.status = 'processed'
                    THEN RAISE(ABORT, 'processed marketplace paid-event inbox row is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN ('received', 'processing', 'retryable_error', 'operator_review', 'processed')
                    THEN RAISE(ABORT, 'invalid marketplace paid-event inbox status') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'processing' AND NEW.lease_owner IS NOT NULL AND trim(NEW.lease_owner) <> '' AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status <> 'processing' AND NEW.lease_owner IS NULL AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace paid-event inbox lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'processed' AND NEW.processed_at IS NOT NULL)
                    OR
                    (NEW.status <> 'processed' AND NEW.processed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace paid-event inbox completion') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(Iden)]
enum MarketplacePaidEventInbox {
    Table,
    Id,
    TenantId,
    EventSource,
    EventId,
    EventHash,
    CheckoutOperationId,
    OrderId,
    PaymentCollectionId,
    CapturedAt,
    CurrencyCode,
    CapturedAmount,
    Status,
    AttemptCount,
    LeaseOwner,
    LeaseExpiresAt,
    LastErrorCode,
    LastErrorMessage,
    CreatedAt,
    UpdatedAt,
    ProcessedAt,
}

#[derive(Iden)]
enum CheckoutOperations {
    Table,
    Id,
}
