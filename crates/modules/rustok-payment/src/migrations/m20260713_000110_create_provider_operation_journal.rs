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
                    .table(PaymentProviderOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PaymentProviderOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::PaymentCollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PaymentProviderOperations::RefundId).uuid())
                    .col(
                        ColumnDef::new(PaymentProviderOperations::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::ProviderId)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::RequestPayload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::ProviderReference)
                            .string_len(191),
                    )
                    .col(ColumnDef::new(PaymentProviderOperations::ProviderResult).json_binary())
                    .col(ColumnDef::new(PaymentProviderOperations::ErrorMessage).string_len(2000))
                    .col(
                        ColumnDef::new(PaymentProviderOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::ProviderCompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(PaymentProviderOperations::CommittedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                PaymentProviderOperations::Table,
                                PaymentProviderOperations::PaymentCollectionId,
                            )
                            .to(PaymentCollections::Table, PaymentCollections::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                PaymentProviderOperations::Table,
                                PaymentProviderOperations::RefundId,
                            )
                            .to(Refunds::Table, Refunds::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_payment_provider_operation_idempotency")
                    .table(PaymentProviderOperations::Table)
                    .col(PaymentProviderOperations::TenantId)
                    .col(PaymentProviderOperations::ProviderId)
                    .col(PaymentProviderOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_provider_operations_reconciliation")
                    .table(PaymentProviderOperations::Table)
                    .col(PaymentProviderOperations::Status)
                    .col(PaymentProviderOperations::UpdatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_payment_provider_operations_collection")
                    .table(PaymentProviderOperations::Table)
                    .col(PaymentProviderOperations::PaymentCollectionId)
                    .col(PaymentProviderOperations::CreatedAt)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE payment_provider_operations
                            ADD CONSTRAINT ck_payment_provider_operations_operation
                            CHECK (operation IN ('authorize', 'capture', 'cancel', 'refund')),
                            ADD CONSTRAINT ck_payment_provider_operations_status
                            CHECK (status IN (
                                'pending',
                                'provider_succeeded',
                                'provider_error',
                                'reconciliation_required',
                                'committed'
                            )),
                            ADD CONSTRAINT ck_payment_provider_operations_identity
                            CHECK (
                                btrim(provider_id) <> ''
                                AND btrim(idempotency_key) <> ''
                            ),
                            ADD CONSTRAINT ck_payment_provider_operations_state
                            CHECK (
                                (status = 'pending'
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
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER payment_provider_operations_state_guard_insert
                        BEFORE INSERT ON payment_provider_operations
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.operation NOT IN ('authorize', 'capture', 'cancel', 'refund')
                                THEN RAISE(ABORT, 'invalid payment provider operation') END;
                            SELECT CASE WHEN NEW.status NOT IN (
                                'pending', 'provider_succeeded', 'provider_error',
                                'reconciliation_required', 'committed'
                            ) THEN RAISE(ABORT, 'invalid payment provider operation status') END;
                            SELECT CASE WHEN trim(NEW.provider_id) = '' OR trim(NEW.idempotency_key) = ''
                                THEN RAISE(ABORT, 'invalid payment provider operation identity') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending'
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
                            ) THEN RAISE(ABORT, 'invalid payment provider operation state') END;
                        END;

                        CREATE TRIGGER payment_provider_operations_state_guard_update
                        BEFORE UPDATE ON payment_provider_operations
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.id IS NOT OLD.id
                                OR NEW.tenant_id IS NOT OLD.tenant_id
                                OR NEW.payment_collection_id IS NOT OLD.payment_collection_id
                                OR NEW.operation IS NOT OLD.operation
                                OR NEW.provider_id IS NOT OLD.provider_id
                                OR NEW.idempotency_key IS NOT OLD.idempotency_key
                                OR NEW.request_payload IS NOT OLD.request_payload
                                THEN RAISE(ABORT, 'payment provider operation identity is immutable') END;
                            SELECT CASE WHEN NEW.status NOT IN (
                                'pending', 'provider_succeeded', 'provider_error',
                                'reconciliation_required', 'committed'
                            ) THEN RAISE(ABORT, 'invalid payment provider operation status') END;
                            SELECT CASE WHEN NOT (
                                (OLD.status = NEW.status)
                                OR (OLD.status IN ('pending', 'provider_error')
                                    AND NEW.status IN ('provider_succeeded', 'provider_error'))
                                OR (OLD.status = 'provider_succeeded'
                                    AND NEW.status IN ('reconciliation_required', 'committed'))
                                OR (OLD.status = 'reconciliation_required'
                                    AND NEW.status = 'committed')
                            ) THEN RAISE(ABORT, 'invalid payment provider operation transition') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending'
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
                            ) THEN RAISE(ABORT, 'invalid payment provider operation state') END;
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
        manager
            .drop_table(
                Table::drop()
                    .table(PaymentProviderOperations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PaymentProviderOperations {
    Table,
    Id,
    TenantId,
    PaymentCollectionId,
    RefundId,
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
enum PaymentCollections {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Refunds {
    Table,
    Id,
}
