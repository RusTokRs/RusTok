use sea_orm_migration::prelude::*;

/// Adds refresh-token theft evidence and an absolute lineage cap to
/// `sessions`:
///
/// - `previous_token_hash` / `previous_token_revoked_at` remember the
///   token rotated away at the last refresh, so replaying it after a
///   short grace window is detectable as theft (the session is then
///   revoked). Previously the hash was overwritten in place and reuse
///   was indistinguishable from a random invalid token.
/// - `absolute_expires_at` caps the whole refresh lineage; the sliding
///   per-rotation expiry can no longer extend a session forever. NULL
///   (pre-migration rows) is coalesced in code to `created_at + cap`.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager
            .has_column("sessions", "previous_token_hash")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("sessions"))
                        .add_column(
                            ColumnDef::new(Alias::new("previous_token_hash")).string(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        if !manager
            .has_column("sessions", "previous_token_revoked_at")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("sessions"))
                        .add_column(
                            ColumnDef::new(Alias::new("previous_token_revoked_at"))
                                .timestamp_with_time_zone(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        if !manager
            .has_column("sessions", "absolute_expires_at")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("sessions"))
                        .add_column(
                            ColumnDef::new(Alias::new("absolute_expires_at"))
                                .timestamp_with_time_zone(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_column("sessions", "absolute_expires_at").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("sessions"))
                        .drop_column(Alias::new("absolute_expires_at"))
                        .to_owned(),
                )
                .await?;
        }

        if manager
            .has_column("sessions", "previous_token_revoked_at")
            .await?
        {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("sessions"))
                        .drop_column(Alias::new("previous_token_revoked_at"))
                        .to_owned(),
                )
                .await?;
        }

        if manager.has_column("sessions", "previous_token_hash").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("sessions"))
                        .drop_column(Alias::new("previous_token_hash"))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
