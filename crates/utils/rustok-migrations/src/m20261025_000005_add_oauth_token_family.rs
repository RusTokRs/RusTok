use sea_orm_migration::prelude::*;

/// Adds refresh-token family tracking to `oauth_tokens`:
///
/// - `family_id` links every rotated successor to the first token of
///   its lineage (the root token's own id). Replaying a rotated-away
///   token after a short grace window is then detectable as theft and
///   the whole family can be revoked, matching RFC 9700 guidance and
///   the existing first-party session reuse detection. The family
///   root's `created_at` also anchors an absolute 90-day lineage cap.
///   NULL (pre-migration rows) is coalesced in code to the row's own
///   id, i.e. every legacy token starts its own family.
#[derive(DeriveMigrationName)]
pub struct Migration;

const FAMILY_INDEX: &str = "idx_oauth_tokens_family_id";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("oauth_tokens", "family_id").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("oauth_tokens"))
                        .add_column(ColumnDef::new(Alias::new("family_id")).uuid())
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_index(
                Index::create()
                    .name(FAMILY_INDEX)
                    .table(Alias::new("oauth_tokens"))
                    .if_not_exists()
                    .col(Alias::new("family_id"))
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(FAMILY_INDEX)
                    .table(Alias::new("oauth_tokens"))
                    .to_owned(),
            )
            .await?;

        if manager.has_column("oauth_tokens", "family_id").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("oauth_tokens"))
                        .drop_column(Alias::new("family_id"))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
