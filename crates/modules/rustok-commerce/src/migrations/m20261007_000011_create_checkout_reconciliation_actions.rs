use sea_orm_migration::prelude::*;

/// Append-only journal of operator decisions on parked checkout operations.
///
/// Every reconciliation action (see `CheckoutReconciliationService`) appends one
/// row: which action ran, who ran it, why, on what evidence, and what it did to
/// the money. The table has no `updated_at` and no update path in the code base,
/// so a row is never rewritten; the current state of an operation stays on
/// `checkout_operations`.
///
/// The unique index on `(tenant_id, checkout_operation_id, idempotency_key)` is
/// what makes a replayed action a no-op instead of a second refund, and it is a
/// declarative schema constraint rather than workflow logic.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CheckoutReconciliationActions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::CheckoutOperationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::CartId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::Action)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::ResultStatus)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CheckoutReconciliationActions::Amount).decimal())
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::CurrencyCode)
                            .string_len(8),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::Reason)
                            .string_len(1500)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::EvidenceRef)
                            .string_len(500),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::OperatorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CheckoutReconciliationActions::ApproverId).uuid())
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::RequestHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CheckoutReconciliationActions::RefundId).uuid())
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::RefundStatus)
                            .string_len(64),
                    )
                    .col(
                        ColumnDef::new(CheckoutReconciliationActions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("ux_checkout_reconciliation_action_idempotency")
                .table(CheckoutReconciliationActions::Table)
                .col(CheckoutReconciliationActions::TenantId)
                .col(CheckoutReconciliationActions::CheckoutOperationId)
                .col(CheckoutReconciliationActions::IdempotencyKey)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_checkout_reconciliation_action_operation")
                .table(CheckoutReconciliationActions::Table)
                .col(CheckoutReconciliationActions::TenantId)
                .col(CheckoutReconciliationActions::CheckoutOperationId)
                .col(CheckoutReconciliationActions::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_checkout_reconciliation_action_operator")
                .table(CheckoutReconciliationActions::Table)
                .col(CheckoutReconciliationActions::TenantId)
                .col(CheckoutReconciliationActions::OperatorId)
                .col(CheckoutReconciliationActions::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for index in [
            Index::drop()
                .name("idx_checkout_reconciliation_action_operator")
                .table(CheckoutReconciliationActions::Table)
                .to_owned(),
            Index::drop()
                .name("idx_checkout_reconciliation_action_operation")
                .table(CheckoutReconciliationActions::Table)
                .to_owned(),
            Index::drop()
                .name("ux_checkout_reconciliation_action_idempotency")
                .table(CheckoutReconciliationActions::Table)
                .to_owned(),
        ] {
            manager.drop_index(index).await?;
        }
        manager
            .drop_table(
                Table::drop()
                    .table(CheckoutReconciliationActions::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum CheckoutReconciliationActions {
    Table,
    Id,
    TenantId,
    CheckoutOperationId,
    CartId,
    Action,
    ResultStatus,
    Amount,
    CurrencyCode,
    Reason,
    EvidenceRef,
    OperatorId,
    ApproverId,
    IdempotencyKey,
    RequestHash,
    RefundId,
    RefundStatus,
    CreatedAt,
}
