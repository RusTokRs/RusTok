use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::Statement;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut amount_decimal = ColumnDef::new(Prices::AmountDecimal);
        amount_decimal.decimal_len(20, 6).not_null().default(0);
        add_column_if_missing(manager, Prices::Table, amount_decimal).await?;

        let mut compare_at_amount_decimal = ColumnDef::new(Prices::CompareAtAmountDecimal);
        compare_at_amount_decimal.decimal_len(20, 6);
        add_column_if_missing(manager, Prices::Table, compare_at_amount_decimal).await?;

        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                "UPDATE prices
                 SET amount_decimal = CAST(amount AS NUMERIC),
                     compare_at_amount_decimal = CASE
                         WHEN compare_at_amount IS NULL THEN NULL
                         ELSE CAST(compare_at_amount AS NUMERIC)
                     END
                 WHERE amount_decimal IS NULL OR amount_decimal = 0"
                    .to_string(),
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_column_if_present(manager, Prices::Table, Prices::CompareAtAmountDecimal).await?;
        drop_column_if_present(manager, Prices::Table, Prices::AmountDecimal).await
    }
}

async fn add_column_if_missing<T>(
    manager: &SchemaManager<'_>,
    table: T,
    column: ColumnDef,
) -> Result<(), DbErr>
where
    T: Iden + 'static,
{
    manager
        .alter_table(
            Table::alter()
                .table(table)
                .add_column_if_not_exists(column)
                .to_owned(),
        )
        .await
}

async fn drop_column_if_present<T, C>(
    manager: &SchemaManager<'_>,
    table: T,
    column: C,
) -> Result<(), DbErr>
where
    T: Iden + 'static,
    C: IntoIden,
{
    manager
        .alter_table(
            Table::alter()
                .table(table)
                .drop_column(column)
                .to_owned(),
        )
        .await
}

#[derive(Iden)]
enum Prices {
    Table,
    AmountDecimal,
    CompareAtAmountDecimal,
}
