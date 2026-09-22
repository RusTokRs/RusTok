use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager
            .has_column("product_field_definitions", "is_localized")
            .await?
        {
            return Ok(());
        }

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("product_field_definitions"))
                    .add_column(
                        ColumnDef::new(Alias::new("is_localized"))
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager
            .has_column("product_field_definitions", "is_localized")
            .await?
        {
            return Ok(());
        }

        manager
            .alter_table(
                Table::alter()
                    .table(Alias::new("product_field_definitions"))
                    .drop_column(Alias::new("is_localized"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}
