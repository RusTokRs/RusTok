use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> { Some(true) }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::create_outbox_dispatch_attempts(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        rustok_outbox::migration::drop_outbox_dispatch_attempts(manager).await
    }
}
