use sea_orm_migration::prelude::*;

/// Registers OAuth access JWTs alongside refresh tokens.
///
/// `token_kind` separates durable access-token registration rows from
/// refresh-token lineage rows. Existing rows are refresh tokens. The
/// nullable `consent_granted_at` captures the consent epoch for delegated
/// rows so a revoke/re-grant cannot resurrect older tokens.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("oauth_tokens", "token_kind").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("oauth_tokens"))
                        .add_column(
                            ColumnDef::new(Alias::new("token_kind"))
                                .string_len(16)
                                .not_null()
                                .default("refresh"),
                        )
                        .to_owned(),
                )
                .await?;
        }
        if !manager
            .has_column("oauth_tokens", "consent_granted_at")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("oauth_tokens"))
                        .add_column(
                            ColumnDef::new(Alias::new("consent_granted_at"))
                                .timestamp_with_time_zone()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_tokens_kind_app_tenant")
                    .table(Alias::new("oauth_tokens"))
                    .if_not_exists()
                    .col(Alias::new("token_kind"))
                    .col(Alias::new("app_id"))
                    .col(Alias::new("tenant_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_oauth_tokens_kind_app_tenant")
                    .table(Alias::new("oauth_tokens"))
                    .to_owned(),
            )
            .await?;
        if manager
            .has_column("oauth_tokens", "consent_granted_at")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("oauth_tokens"))
                        .drop_column(Alias::new("consent_granted_at"))
                        .to_owned(),
                )
                .await?;
        }
        if manager.has_column("oauth_tokens", "token_kind").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("oauth_tokens"))
                        .drop_column(Alias::new("token_kind"))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
