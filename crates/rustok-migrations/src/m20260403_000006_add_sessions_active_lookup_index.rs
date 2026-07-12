use sea_orm_migration::prelude::*;

use super::m20250101_000004_create_sessions::Sessions;

#[derive(DeriveMigrationName)]
pub struct Migration;

const ACTIVE_LOOKUP_INDEX: &str = "idx_sessions_active_lookup";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name(ACTIVE_LOOKUP_INDEX)
                    .table(Sessions::Table)
                    .if_not_exists()
                    .col(Sessions::TenantId)
                    .col(Sessions::UserId)
                    .col(Sessions::RevokedAt)
                    .col(Sessions::ExpiresAt)
                    .col(Sessions::CreatedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name(ACTIVE_LOOKUP_INDEX)
                    .table(Sessions::Table)
                    .to_owned(),
            )
            .await
    }
}
