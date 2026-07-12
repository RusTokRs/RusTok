use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("ux_payments_collection")
                    .table(Payments::Table)
                    .col(Payments::PaymentCollectionId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE UNIQUE INDEX IF NOT EXISTS ux_payment_collections_active_cart
                        ON payment_collections (tenant_id, cart_id)
                        WHERE cart_id IS NOT NULL
                          AND status IN ('pending', 'authorized', 'captured')
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                // MySQL does not support partial indexes. Runtime row locking still
                // protects the lifecycle there; the portable one-payment invariant
                // above remains enforced on every backend.
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !matches!(manager.get_database_backend(), DatabaseBackend::MySql) {
            manager
                .drop_index(
                    Index::drop()
                        .name("ux_payment_collections_active_cart")
                        .table(PaymentCollections::Table)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .drop_index(
                Index::drop()
                    .name("ux_payments_collection")
                    .table(Payments::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PaymentCollections {
    Table,
}

#[derive(DeriveIden)]
enum Payments {
    Table,
    PaymentCollectionId,
}
