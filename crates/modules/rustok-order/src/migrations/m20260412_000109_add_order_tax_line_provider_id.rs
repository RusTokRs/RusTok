use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderTaxLines::Table)
                    .add_column(
                        ColumnDef::new(OrderTaxLines::ProviderId)
                            .string_len(64)
                            .not_null()
                            .default("region_default"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrderTaxLines::Table)
                    .drop_column(OrderTaxLines::ProviderId)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum OrderTaxLines {
    Table,
    ProviderId,
}
