use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MarketplaceSellers::Table)
                    .drop_column(MarketplaceSellers::OnboardingNote)
                    .drop_column(MarketplaceSellers::SuspensionReason)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(MarketplaceSellers::Table)
                    .add_column(ColumnDef::new(MarketplaceSellers::OnboardingNote).text().null())
                    .add_column(ColumnDef::new(MarketplaceSellers::SuspensionReason).text().null())
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum MarketplaceSellers {
    Table,
    OnboardingNote,
    SuspensionReason,
}
