use sea_orm_migration::prelude::*;

/// Adds the index the admin reconciliation list reads through.
///
/// `CheckoutOperationJournal::list_by_status` selects the newest rows of one
/// tenant (`WHERE tenant_id = ? [AND status = ?] ORDER BY updated_at DESC LIMIT
/// n`). The indexes the table carried until now are keyed on `cart_id`
/// (`idx_checkout_operations_cart`), on the idempotency scope
/// (`ux_checkout_operations_idempotency`), on the compensation lease
/// (`idx_checkout_operations_recovery`) and on the active-cart slot
/// (`ux_checkout_operations_active_cart`), so the operator list fell back to a
/// scan of every operation of every tenant plus a sort for the requested page.
///
/// The index is tenant-leading and keeps the filtered list (the reconciliation
/// queue asks for one status) on one ordered range. The unfiltered variant still
/// sorts the tenant's rows by `updated_at`; that path is bounded by one tenant's
/// history and is left to the planner instead of paying for a second index on
/// every checkout write.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_checkout_operations_tenant_status_updated")
                    .table(CheckoutOperations::Table)
                    .col(CheckoutOperations::TenantId)
                    .col(CheckoutOperations::Status)
                    .col(CheckoutOperations::UpdatedAt)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .if_exists()
                    .name("idx_checkout_operations_tenant_status_updated")
                    .table(CheckoutOperations::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum CheckoutOperations {
    Table,
    TenantId,
    Status,
    UpdatedAt,
}
