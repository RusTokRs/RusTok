use super::m20260308_000001_create_oauth_apps::OAuthApps;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OAuthApps::Table)
                    .add_column(
                        ColumnDef::new(OAuthApps::GrantedPermissions)
                            .json_binary()
                            .not_null()
                            .default("[]"),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OAuthApps::Table)
                    .drop_column(OAuthApps::GrantedPermissions)
                    .to_owned(),
            )
            .await
    }
}
