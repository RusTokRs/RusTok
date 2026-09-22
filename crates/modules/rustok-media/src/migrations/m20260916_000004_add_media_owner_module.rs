use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Assets::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(Assets::OwnerModule)
                            .string_len(64)
                            .not_null()
                            .default("general"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_media_assets_tenant_owner_module")
                    .table(Assets::Table)
                    .col(Assets::TenantId)
                    .col(Assets::OwnerModule)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_media_assets_tenant_owner_module")
                    .table(Assets::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Assets::Table)
                    .drop_column(Assets::OwnerModule)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }
}

#[derive(Iden)]
enum Assets {
    #[iden = "media_assets"]
    Table,
    TenantId,
    OwnerModule,
}
