use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SearchSettings::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SearchSettings::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SearchSettings::TenantId).uuid())
                    .col(
                        ColumnDef::new(SearchSettings::ActiveEngine)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchSettings::FallbackEngine)
                            .string()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchSettings::Config)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SearchSettings::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_search_settings_tenant")
                    .table(SearchSettings::Table)
                    .col(SearchSettings::TenantId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_search_settings_tenant")
                    .table(SearchSettings::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_table(Table::drop().table(SearchSettings::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum SearchSettings {
    Table,
    Id,
    TenantId,
    ActiveEngine,
    FallbackEngine,
    Config,
    UpdatedAt,
}
