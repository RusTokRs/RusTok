use sea_orm_migration::prelude::*;

use super::m20250101_000001_create_tenants::Tenants;
use super::m20250101_000002_create_users::Users;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Tenants::Table)
                    .add_column(
                        ColumnDef::new(Tenants::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_tenants_metadata")
                    .table(Tenants::Table)
                    .col(Tenants::Metadata)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_users_metadata")
                    .table(Users::Table)
                    .col(Users::Metadata)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_users_metadata")
                    .table(Users::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_tenants_metadata")
                    .table(Tenants::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(Tenants::Table)
                    .drop_column(Tenants::Metadata)
                    .to_owned(),
            )
            .await
    }
}
