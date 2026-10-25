use sea_orm_migration::prelude::*;

use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PlatformState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PlatformState::Id)
                            .string_len(32)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PlatformState::Revision)
                            .big_integer()
                            .not_null()
                            .default(1),
                    )
                    .col(
                        ColumnDef::new(PlatformState::ManifestJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PlatformState::ManifestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PlatformState::UpdatedBy).string_len(255))
                    .col(
                        ColumnDef::new(PlatformState::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PlatformState::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(Builds::Table)
                        .add_column(
                            ColumnDef::new(Builds::ManifestRevision)
                                .big_integer()
                                .not_null()
                                .default(0),
                        )
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Builds::Table)
                        .add_column(
                            ColumnDef::new(Builds::ManifestSnapshot)
                                .json_binary()
                                .not_null()
                                .default("{}"),
                        )
                        .to_owned(),
                )
                .await?;
            Ok(())
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Builds::Table)
                        .add_column(
                            ColumnDef::new(Builds::ManifestRevision)
                                .big_integer()
                                .not_null()
                                .default(0),
                        )
                        .add_column(
                            ColumnDef::new(Builds::ManifestSnapshot)
                                .json_binary()
                                .not_null()
                                .default("{}"),
                        )
                        .to_owned(),
                )
                .await?;

            Ok(())
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .alter_table(
                    Table::alter()
                        .table(Builds::Table)
                        .drop_column(Builds::ManifestSnapshot)
                        .to_owned(),
                )
                .await?;
            manager
                .alter_table(
                    Table::alter()
                        .table(Builds::Table)
                        .drop_column(Builds::ManifestRevision)
                        .to_owned(),
                )
                .await?;
        } else {
            manager
                .alter_table(
                    Table::alter()
                        .table(Builds::Table)
                        .drop_column(Builds::ManifestSnapshot)
                        .drop_column(Builds::ManifestRevision)
                        .to_owned(),
                )
                .await?;
        }
        manager
            .drop_table(Table::drop().table(PlatformState::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
enum PlatformState {
    Table,
    Id,
    Revision,
    ManifestJson,
    ManifestHash,
    UpdatedBy,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Builds {
    Table,
    ManifestRevision,
    ManifestSnapshot,
}
