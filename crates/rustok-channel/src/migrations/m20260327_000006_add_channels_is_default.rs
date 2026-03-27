use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Channels::Table)
                    .add_column(
                        ColumnDef::new(Channels::IsDefault)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .get_connection()
            .execute(Statement::from_string(
                manager.get_database_backend(),
                r#"
                UPDATE channels AS channel
                SET is_default = true
                WHERE channel.id = (
                    SELECT candidate.id
                    FROM channels AS candidate
                    WHERE candidate.tenant_id = channel.tenant_id
                      AND candidate.is_active = true
                    ORDER BY candidate.created_at ASC, candidate.id ASC
                    LIMIT 1
                )
                "#,
            ))
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Channels::Table)
                    .drop_column(Channels::IsDefault)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum Channels {
    Table,
    IsDefault,
}
