use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ProductVariants::InventoryPolicy)
                            .string_len(32)
                            .not_null()
                            .default("deny"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ProductVariants::InventoryManagement)
                            .string_len(32)
                            .not_null()
                            .default("rustok"),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ProductVariants::InventoryQuantity)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ProductVariants::WeightUnit).string_len(16),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ProductVariants::CombinationIdentity).text(),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(ProductVariants::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_variants_tenant_id")
                    .table(ProductVariants::Table)
                    .col(ProductVariants::TenantId)
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    CREATE UNIQUE INDEX IF NOT EXISTS uq_product_variants_combination
                        ON product_variants (product_id, combination_identity)
                        WHERE combination_identity IS NOT NULL;
                    CREATE UNIQUE INDEX IF NOT EXISTS uq_product_variants_default
                        ON product_variants (product_id)
                        WHERE combination_identity IS NULL;
                    "#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    DROP INDEX IF EXISTS uq_product_variants_default;
                    DROP INDEX IF EXISTS uq_product_variants_combination;
                    "#,
                )
                .await?;
        }

        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .drop_column(ProductVariants::InventoryPolicy)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .drop_column(ProductVariants::InventoryManagement)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .drop_column(ProductVariants::InventoryQuantity)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .drop_column(ProductVariants::WeightUnit)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .drop_column(ProductVariants::CombinationIdentity)
                    .to_owned(),
            )
            .await?;
        manager
            .alter_table(
                Table::alter()
                    .table(ProductVariants::Table)
                    .drop_column(ProductVariants::Position)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ProductVariants {
    Table,
    TenantId,
    InventoryPolicy,
    InventoryManagement,
    InventoryQuantity,
    WeightUnit,
    CombinationIdentity,
    Position,
}
