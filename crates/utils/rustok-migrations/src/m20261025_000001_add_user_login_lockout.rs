use sea_orm_migration::prelude::*;

/// Adds per-account brute-force protection state to `users`:
/// `failed_login_count` counts consecutive failed credential checks and
/// `locked_until` holds the lock expiry once the threshold is reached.
/// Unlike IP-keyed rate limiting, this protects accounts across every
/// transport (REST, GraphQL, native) and every source address.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("users", "failed_login_count").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("users"))
                        .add_column(
                            ColumnDef::new(Alias::new("failed_login_count"))
                                .integer()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await?;
        }

        if !manager.has_column("users", "locked_until").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("users"))
                        .add_column(
                            ColumnDef::new(Alias::new("locked_until")).timestamp_with_time_zone(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_column("users", "locked_until").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("users"))
                        .drop_column(Alias::new("locked_until"))
                        .to_owned(),
                )
                .await?;
        }

        if manager.has_column("users", "failed_login_count").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("users"))
                        .drop_column(Alias::new("failed_login_count"))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
