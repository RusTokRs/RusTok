use sea_orm_migration::prelude::*;

/// Adds a delegated-credential revocation epoch to `users`.
///
/// First-party sessions keep their existing revocation lineage. Delegated
/// OAuth access tokens carry only second-granularity `iat`, so validators
/// reject tokens issued at or before this instant after a password change.
/// The column is nullable for pre-existing accounts and is written in the
/// same transaction as the password update and OAuth refresh-token revoke.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_column("users", "password_changed_at").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("users"))
                        .add_column(
                            ColumnDef::new(Alias::new("password_changed_at"))
                                .timestamp_with_time_zone(),
                        )
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.has_column("users", "password_changed_at").await? {
            manager
                .alter_table(
                    Table::alter()
                        .table(Alias::new("users"))
                        .drop_column(Alias::new("password_changed_at"))
                        .to_owned(),
                )
                .await?;
        }

        Ok(())
    }
}
