use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let connection = manager.get_connection();
        connection
            .execute(Statement::from_string(
                connection.get_database_backend(),
                "UPDATE page_bodies SET format = 'grapesjs' WHERE LOWER(format) = 'grapesjs_v1'"
                    .to_string(),
            ))
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
