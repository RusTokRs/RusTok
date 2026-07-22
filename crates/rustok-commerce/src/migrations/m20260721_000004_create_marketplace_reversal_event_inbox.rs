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
                    .table(MarketplaceReversalEventInbox::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::ProviderEventId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::EventSource)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::EventId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::EventHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::ReversalKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::SourceId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::OrderId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::PaymentCollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::OccurredAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::CurrencyExponent)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::TotalAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::LinesJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::LeaseOwner)
                            .string_len(191),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(MarketplaceReversalEventInbox::ReversalId).uuid())
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::LedgerTransactionId)
                            .uuid(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::LastErrorCode)
                            .string_len(100),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::LastErrorMessage)
                            .string_len(2000),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplaceReversalEventInbox::ProcessedAt)
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("ux_marketplace_reversal_event_source")
                .table(MarketplaceReversalEventInbox::Table)
                .col(MarketplaceReversalEventInbox::TenantId)
                .col(MarketplaceReversalEventInbox::EventSource)
                .col(MarketplaceReversalEventInbox::EventId)
                .unique()
                .to_owned(),
            Index::create()
                .name("ux_marketplace_reversal_provider_event")
                .table(MarketplaceReversalEventInbox::Table)
                .col(MarketplaceReversalEventInbox::TenantId)
                .col(MarketplaceReversalEventInbox::ProviderEventId)
                .unique()
                .to_owned(),
            Index::create()
                .name("ux_marketplace_reversal_source_identity")
                .table(MarketplaceReversalEventInbox::Table)
                .col(MarketplaceReversalEventInbox::TenantId)
                .col(MarketplaceReversalEventInbox::ReversalKind)
                .col(MarketplaceReversalEventInbox::SourceId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_marketplace_reversal_event_recovery")
                .table(MarketplaceReversalEventInbox::Table)
                .col(MarketplaceReversalEventInbox::Status)
                .col(MarketplaceReversalEventInbox::LeaseExpiresAt)
                .col(MarketplaceReversalEventInbox::UpdatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_reversal_event_order")
                .table(MarketplaceReversalEventInbox::Table)
                .col(MarketplaceReversalEventInbox::TenantId)
                .col(MarketplaceReversalEventInbox::OrderId)
                .col(MarketplaceReversalEventInbox::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

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
                    .table(MarketplaceReversalEventInbox::Table)
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
            ALTER TABLE marketplace_reversal_event_inbox
                ADD CONSTRAINT ck_marketplace_reversal_event_status
                CHECK (status IN ('received', 'processing', 'retryable_error', 'operator_review', 'processed')),
                ADD CONSTRAINT ck_marketplace_reversal_event_identity
                CHECK (
                    btrim(event_source) <> ''
                    AND btrim(event_id) <> ''
                    AND event_hash ~ '^[0-9a-f]{64}$'
                    AND reversal_kind IN ('refund', 'chargeback')
                    AND currency_code ~ '^[A-Z]{3}$'
                    AND currency_exponent BETWEEN 0 AND 9
                    AND total_amount > 0
                    AND jsonb_typeof(lines_json) = 'array'
                    AND jsonb_array_length(lines_json) > 0
                    AND attempt_count >= 0
                ),
                ADD CONSTRAINT ck_marketplace_reversal_event_lease
                CHECK (
                    (status = 'processing' AND lease_owner IS NOT NULL AND btrim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
                    OR
                    (status <> 'processing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
                ),
                ADD CONSTRAINT ck_marketplace_reversal_event_completion
                CHECK (
                    (status = 'processed' AND processed_at IS NOT NULL AND reversal_id IS NOT NULL AND ledger_transaction_id IS NOT NULL)
                    OR
                    (status <> 'processed' AND processed_at IS NULL AND reversal_id IS NULL AND ledger_transaction_id IS NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_marketplace_reversal_event_inbox_integrity()
            RETURNS trigger AS $$
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.provider_event_id IS DISTINCT FROM OLD.provider_event_id
                    OR NEW.event_source IS DISTINCT FROM OLD.event_source
                    OR NEW.event_id IS DISTINCT FROM OLD.event_id
                    OR NEW.event_hash IS DISTINCT FROM OLD.event_hash
                    OR NEW.reversal_kind IS DISTINCT FROM OLD.reversal_kind
                    OR NEW.source_id IS DISTINCT FROM OLD.source_id
                    OR NEW.order_id IS DISTINCT FROM OLD.order_id
                    OR NEW.payment_collection_id IS DISTINCT FROM OLD.payment_collection_id
                    OR NEW.occurred_at IS DISTINCT FROM OLD.occurred_at
                    OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
                    OR NEW.currency_exponent IS DISTINCT FROM OLD.currency_exponent
                    OR NEW.total_amount IS DISTINCT FROM OLD.total_amount
                    OR NEW.lines_json IS DISTINCT FROM OLD.lines_json
                ) THEN
                    RAISE EXCEPTION 'marketplace reversal normalized facts are immutable'
                        USING ERRCODE = '23514';
                END IF;
                IF TG_OP = 'UPDATE' AND OLD.status = 'processed' AND NEW IS DISTINCT FROM OLD THEN
                    RAISE EXCEPTION 'processed marketplace reversal inbox row is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER marketplace_reversal_event_inbox_integrity_guard
            BEFORE UPDATE ON marketplace_reversal_event_inbox
            FOR EACH ROW EXECUTE FUNCTION enforce_marketplace_reversal_event_inbox_integrity();
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
            CREATE TRIGGER marketplace_reversal_event_inbox_guard_insert
            BEFORE INSERT ON marketplace_reversal_event_inbox
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.status NOT IN ('received', 'processing', 'retryable_error', 'operator_review', 'processed')
                    THEN RAISE(ABORT, 'invalid marketplace reversal inbox status') END;
                SELECT CASE WHEN trim(NEW.event_source) = '' OR trim(NEW.event_id) = ''
                    OR length(NEW.event_hash) <> 64
                    OR NEW.reversal_kind NOT IN ('refund', 'chargeback')
                    OR length(NEW.currency_code) <> 3 OR NEW.currency_code <> upper(NEW.currency_code)
                    OR NEW.currency_exponent < 0 OR NEW.currency_exponent > 9
                    OR NEW.total_amount <= 0 OR json_type(NEW.lines_json) <> 'array'
                    OR json_array_length(NEW.lines_json) = 0 OR NEW.attempt_count < 0
                    THEN RAISE(ABORT, 'invalid marketplace reversal inbox identity') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'processing' AND NEW.lease_owner IS NOT NULL AND trim(NEW.lease_owner) <> '' AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status <> 'processing' AND NEW.lease_owner IS NULL AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace reversal inbox lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'processed' AND NEW.processed_at IS NOT NULL AND NEW.reversal_id IS NOT NULL AND NEW.ledger_transaction_id IS NOT NULL)
                    OR
                    (NEW.status <> 'processed' AND NEW.processed_at IS NULL AND NEW.reversal_id IS NULL AND NEW.ledger_transaction_id IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace reversal inbox completion') END;
            END;

            CREATE TRIGGER marketplace_reversal_event_inbox_guard_update
            BEFORE UPDATE ON marketplace_reversal_event_inbox
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.provider_event_id IS NOT OLD.provider_event_id
                    OR NEW.event_source IS NOT OLD.event_source
                    OR NEW.event_id IS NOT OLD.event_id
                    OR NEW.event_hash IS NOT OLD.event_hash
                    OR NEW.reversal_kind IS NOT OLD.reversal_kind
                    OR NEW.source_id IS NOT OLD.source_id
                    OR NEW.order_id IS NOT OLD.order_id
                    OR NEW.payment_collection_id IS NOT OLD.payment_collection_id
                    OR NEW.occurred_at IS NOT OLD.occurred_at
                    OR NEW.currency_code IS NOT OLD.currency_code
                    OR NEW.currency_exponent IS NOT OLD.currency_exponent
                    OR NEW.total_amount IS NOT OLD.total_amount
                    OR NEW.lines_json IS NOT OLD.lines_json
                    THEN RAISE(ABORT, 'marketplace reversal normalized facts are immutable') END;
                SELECT CASE WHEN OLD.status = 'processed'
                    THEN RAISE(ABORT, 'processed marketplace reversal inbox row is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN ('received', 'processing', 'retryable_error', 'operator_review', 'processed')
                    THEN RAISE(ABORT, 'invalid marketplace reversal inbox status') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'processing' AND NEW.lease_owner IS NOT NULL AND trim(NEW.lease_owner) <> '' AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status <> 'processing' AND NEW.lease_owner IS NULL AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace reversal inbox lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'processed' AND NEW.processed_at IS NOT NULL AND NEW.reversal_id IS NOT NULL AND NEW.ledger_transaction_id IS NOT NULL)
                    OR
                    (NEW.status <> 'processed' AND NEW.processed_at IS NULL AND NEW.reversal_id IS NULL AND NEW.ledger_transaction_id IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace reversal inbox completion') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(Iden)]
enum MarketplaceReversalEventInbox {
    Table,
    Id,
    TenantId,
    ProviderEventId,
    EventSource,
    EventId,
    EventHash,
    ReversalKind,
    SourceId,
    OrderId,
    PaymentCollectionId,
    OccurredAt,
    CurrencyCode,
    CurrencyExponent,
    TotalAmount,
    LinesJson,
    Status,
    AttemptCount,
    LeaseOwner,
    LeaseExpiresAt,
    ReversalId,
    LedgerTransactionId,
    LastErrorCode,
    LastErrorMessage,
    CreatedAt,
    UpdatedAt,
    ProcessedAt,
}
