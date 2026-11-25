use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(Iden)]
enum History {
    #[iden = "session_refresh_history"]
    Table,
    TokenHash,
    SessionId,
    RotatedAt,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(History::Table)
                    .col(
                        ColumnDef::new(History::TokenHash)
                            .string_len(255)
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(History::SessionId).uuid().not_null())
                    .col(ColumnDef::new(History::RotatedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_session_refresh_history_session")
                            .from(History::Table, History::SessionId)
                            .to(Alias::new("sessions"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_session_refresh_history_session")
                    .table(History::Table)
                    .col(History::SessionId)
                    .to_owned(),
            )
            .await?;
        // Preserve the exact available evidence. A missing rotation timestamp
        // stays NULL and is never eligible for the replay grace period.
        manager
            .get_connection()
            .execute_unprepared(
                "INSERT INTO session_refresh_history (token_hash, session_id, rotated_at) \
                 SELECT previous_token_hash, id, previous_token_revoked_at FROM sessions \
                 WHERE previous_token_hash IS NOT NULL",
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(History::Table).to_owned())
            .await
    }
}
