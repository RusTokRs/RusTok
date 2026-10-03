use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(FulfillmentCheckoutIdentities::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(FulfillmentCheckoutIdentities::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentCheckoutIdentities::CheckoutOperationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentCheckoutIdentities::OrderId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(FulfillmentCheckoutIdentities::CustomerId).uuid())
                    .col(
                        ColumnDef::new(FulfillmentCheckoutIdentities::PlanHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(FulfillmentCheckoutIdentities::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(FulfillmentCheckoutIdentities::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(FulfillmentCheckoutIdentities::TenantId)
                            .col(FulfillmentCheckoutIdentities::CheckoutOperationId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                INSERT INTO fulfillment_checkout_identities (
                    tenant_id,
                    checkout_operation_id,
                    order_id,
                    customer_id,
                    plan_hash,
                    created_at,
                    updated_at
                )
                SELECT
                    tenant_id,
                    checkout_operation_id,
                    order_id,
                    customer_id,
                    LOWER(checkout_plan_hash) AS plan_hash,
                    MIN(created_at) AS created_at,
                    MIN(updated_at) AS updated_at
                FROM fulfillments
                WHERE checkout_operation_id IS NOT NULL
                  AND checkout_fulfillment_index IS NOT NULL
                  AND checkout_plan_hash IS NOT NULL
                GROUP BY tenant_id, checkout_operation_id, order_id, customer_id, LOWER(checkout_plan_hash);
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(FulfillmentCheckoutIdentities::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum FulfillmentCheckoutIdentities {
    Table,
    TenantId,
    CheckoutOperationId,
    OrderId,
    CustomerId,
    PlanHash,
    CreatedAt,
    UpdatedAt,
}
