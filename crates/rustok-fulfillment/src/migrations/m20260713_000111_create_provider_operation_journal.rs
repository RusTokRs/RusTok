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
                    .table(FulfillmentProviderOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::FulfillmentId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::ProviderId)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::RequestPayload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::ProviderReference)
                            .string_len(191),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::ProviderResult).json_binary(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::ErrorMessage)
                            .string_len(2000),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::ProviderCompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentProviderOperations::CommittedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                FulfillmentProviderOperations::Table,
                                FulfillmentProviderOperations::FulfillmentId,
                            )
                            .to(Fulfillments::Table, Fulfillments::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_fulfillment_provider_operation_idempotency")
                    .table(FulfillmentProviderOperations::Table)
                    .col(FulfillmentProviderOperations::TenantId)
                    .col(FulfillmentProviderOperations::ProviderId)
                    .col(FulfillmentProviderOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_fulfillment_provider_operations_reconciliation")
                    .table(FulfillmentProviderOperations::Table)
                    .col(FulfillmentProviderOperations::Status)
                    .col(FulfillmentProviderOperations::UpdatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_fulfillment_provider_operations_fulfillment")
                    .table(FulfillmentProviderOperations::Table)
                    .col(FulfillmentProviderOperations::FulfillmentId)
                    .col(FulfillmentProviderOperations::CreatedAt)
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
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    DROP TRIGGER IF EXISTS fulfillment_provider_operations_lifecycle_guard
                        ON fulfillment_provider_operations;
                    DROP FUNCTION IF EXISTS enforce_fulfillment_provider_operation_lifecycle();
                    "#,
                )
                .await?;
        }
        manager
            .drop_table(
                Table::drop()
                    .table(FulfillmentProviderOperations::Table)
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
            ALTER TABLE fulfillment_provider_operations
                ADD CONSTRAINT ck_fulfillment_provider_operations_operation
                CHECK (operation IN ('create_label', 'ship', 'reship', 'cancel')),
                ADD CONSTRAINT ck_fulfillment_provider_operations_status
                CHECK (status IN (
                    'pending', 'executing', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                )),
                ADD CONSTRAINT ck_fulfillment_provider_operations_identity
                CHECK (btrim(provider_id) <> '' AND btrim(idempotency_key) <> ''),
                ADD CONSTRAINT ck_fulfillment_provider_operations_state
                CHECK (
                    (status IN ('pending', 'executing')
                        AND provider_completed_at IS NULL
                        AND committed_at IS NULL)
                    OR
                    (status = 'provider_error'
                        AND error_message IS NOT NULL
                        AND committed_at IS NULL)
                    OR
                    (status IN ('provider_succeeded', 'reconciliation_required')
                        AND provider_completed_at IS NOT NULL
                        AND committed_at IS NULL)
                    OR
                    (status = 'committed'
                        AND provider_completed_at IS NOT NULL
                        AND committed_at IS NOT NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_fulfillment_provider_operation_lifecycle()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                   OR NEW.fulfillment_id IS DISTINCT FROM OLD.fulfillment_id
                   OR NEW.operation IS DISTINCT FROM OLD.operation
                   OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
                   OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                   OR NEW.request_payload IS DISTINCT FROM OLD.request_payload THEN
                    RAISE EXCEPTION 'fulfillment provider operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.status IS DISTINCT FROM OLD.status
                   AND NOT (
                        (OLD.status IN ('pending', 'provider_error') AND NEW.status = 'executing')
                        OR (OLD.status = 'executing'
                            AND NEW.status IN ('provider_succeeded', 'provider_error'))
                        OR (OLD.status = 'provider_succeeded'
                            AND NEW.status IN ('reconciliation_required', 'committed'))
                        OR (OLD.status = 'reconciliation_required' AND NEW.status = 'committed')
                   ) THEN
                    RAISE EXCEPTION 'invalid fulfillment provider operation transition from % to %',
                        OLD.status, NEW.status
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillment_provider_operations_lifecycle_guard
            BEFORE UPDATE ON fulfillment_provider_operations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_provider_operation_lifecycle();
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
            CREATE TRIGGER fulfillment_provider_operations_state_guard_insert
            BEFORE INSERT ON fulfillment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.operation NOT IN ('create_label', 'ship', 'reship', 'cancel')
                    THEN RAISE(ABORT, 'invalid fulfillment provider operation') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation status') END;
                SELECT CASE WHEN trim(NEW.provider_id) = '' OR trim(NEW.idempotency_key) = ''
                    THEN RAISE(ABORT, 'invalid fulfillment provider operation identity') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('pending', 'executing')
                        AND NEW.provider_completed_at IS NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'provider_error'
                        AND NEW.error_message IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status IN ('provider_succeeded', 'reconciliation_required')
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'committed'
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation state') END;
            END;

            CREATE TRIGGER fulfillment_provider_operations_state_guard_update
            BEFORE UPDATE ON fulfillment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.fulfillment_id IS NOT OLD.fulfillment_id
                    OR NEW.operation IS NOT OLD.operation
                    OR NEW.provider_id IS NOT OLD.provider_id
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_payload IS NOT OLD.request_payload
                    THEN RAISE(ABORT, 'fulfillment provider operation identity is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation status') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'provider_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing'
                        AND NEW.status IN ('provider_succeeded', 'provider_error'))
                    OR (OLD.status = 'provider_succeeded'
                        AND NEW.status IN ('reconciliation_required', 'committed'))
                    OR (OLD.status = 'reconciliation_required' AND NEW.status = 'committed')
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation transition') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('pending', 'executing')
                        AND NEW.provider_completed_at IS NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'provider_error'
                        AND NEW.error_message IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status IN ('provider_succeeded', 'reconciliation_required')
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'committed'
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation state') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum FulfillmentProviderOperations {
    Table,
    Id,
    TenantId,
    FulfillmentId,
    Operation,
    ProviderId,
    IdempotencyKey,
    Status,
    RequestPayload,
    ProviderReference,
    ProviderResult,
    ErrorMessage,
    CreatedAt,
    UpdatedAt,
    ProviderCompletedAt,
    CommittedAt,
}

#[derive(DeriveIden)]
enum Fulfillments {
    Table,
    Id,
}
