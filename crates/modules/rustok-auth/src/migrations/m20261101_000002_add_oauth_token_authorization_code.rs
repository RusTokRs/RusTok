use sea_orm_migration::prelude::*;

/// Links a delegated OAuth token lineage to the authorization code that
/// originally issued it. Replay of that code can then revoke every refresh
/// row descended from the code without guessing from token hashes.
///
/// Existing rows and service-client tokens keep NULL. The column is nullable
/// and additive so the migration is reversible without rewriting history.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager
            .has_column("oauth_tokens", "authorization_code_id")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("oauth_tokens"))
                        .add_column(
                            ColumnDef::new(Alias::new("authorization_code_id"))
                                .uuid()
                                .null(),
                        )
                        .to_owned(),
                )
                .await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("idx_oauth_tokens_authorization_code_id")
                    .table(Alias::new("oauth_tokens"))
                    .if_not_exists()
                    .col(Alias::new("authorization_code_id"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("idx_oauth_tokens_authorization_code_id")
                    .table(Alias::new("oauth_tokens"))
                    .to_owned(),
            )
            .await?;
        if manager
            .has_column("oauth_tokens", "authorization_code_id")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("oauth_tokens"))
                        .drop_column(Alias::new("authorization_code_id"))
                        .to_owned(),
                )
                .await?;
        }
        Ok(())
    }
}
