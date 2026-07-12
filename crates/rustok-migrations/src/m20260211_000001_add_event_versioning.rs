use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // This migration is kept for compatibility with historical migration chains,
        // but `sys_events` is now created with these columns already present.
        // When the table does not exist yet (fresh installs), this step must be a no-op.
        if !manager.has_table("sys_events").await? {
            return Ok(());
        }

        manager
            .alter_table(
                Table::alter()
                    .table(SysEvents::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(SysEvents::EventType)
                            .string_len(128)
                            .not_null()
                            .default("unknown"),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(SysEvents::Table)
                    .add_column_if_not_exists(
                        ColumnDef::new(SysEvents::SchemaVersion)
                            .small_integer()
                            .not_null()
                            .default(1),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sys_events_type_version")
                    .table(SysEvents::Table)
                    .col(SysEvents::EventType)
                    .col(SysEvents::SchemaVersion)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if !manager.has_table("sys_events").await? {
            return Ok(());
        }

        // Drop index first
        manager
            .drop_index(
                Index::drop()
                    .name("idx_sys_events_type_version")
                    .table(SysEvents::Table)
                    .to_owned(),
            )
            .await?;

        // Drop columns
        manager
            .alter_table(
                Table::alter()
                    .table(SysEvents::Table)
                    .drop_column(SysEvents::SchemaVersion)
                    .drop_column(SysEvents::EventType)
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum SysEvents {
    Table,
    EventType,
    SchemaVersion,
}
