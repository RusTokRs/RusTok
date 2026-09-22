use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let mut rule_kind = ColumnDef::new(PriceLists::RuleKind);
        rule_kind.string_len(32);
        add_column_if_missing(manager, PriceLists::Table, rule_kind).await?;

        let mut adjustment_percent = ColumnDef::new(PriceLists::AdjustmentPercent);
        adjustment_percent.decimal_len(16, 6);
        add_column_if_missing(manager, PriceLists::Table, adjustment_percent).await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        drop_column_if_present(manager, PriceLists::Table, PriceLists::AdjustmentPercent).await?;
        drop_column_if_present(manager, PriceLists::Table, PriceLists::RuleKind).await
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
        .alter_table(Table::alter().table(table).drop_column(column).to_owned())
        .await
}

#[derive(Iden)]
enum PriceLists {
    Table,
    RuleKind,
    AdjustmentPercent,
}
