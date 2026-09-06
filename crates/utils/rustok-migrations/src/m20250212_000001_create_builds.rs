use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Builds::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Builds::Id).uuid().not_null().primary_key())
                    .col(
                        ColumnDef::new(Builds::Status)
                            .string_len(32)
                            .not_null()
                            .default("queued"),
                    )
                    .col(
                        ColumnDef::new(Builds::Stage)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(
                        ColumnDef::new(Builds::Progress)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Builds::Profile)
                            .string_len(32)
                            .not_null()
                            .default("monolith"),
                    )
                    .col(
                        ColumnDef::new(Builds::ManifestRef)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Builds::ManifestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Builds::ModulesDelta).json_binary())
                    .col(
                        ColumnDef::new(Builds::RequestedBy)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Builds::Reason).text())
                    .col(ColumnDef::new(Builds::LogsUrl).text())
                    .col(ColumnDef::new(Builds::ErrorMessage).text())
                    .col(ColumnDef::new(Builds::StartedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Builds::FinishedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Builds::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Builds::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_builds_manifest_hash")
                    .table(Builds::Table)
                    .col(Builds::ManifestHash)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_builds_status")
                    .table(Builds::Table)
                    .col(Builds::Status)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Builds::Table).to_owned())
            .await
    }
}

#[derive(Iden)]
pub enum Builds {
    Table,
    Id,
    Status,
    Stage,
    Progress,
    Profile,
    ManifestRef,
    ManifestHash,
    ModulesDelta,
    RequestedBy,
    Reason,
    LogsUrl,
    ErrorMessage,
    StartedAt,
    FinishedAt,
    CreatedAt,
    UpdatedAt,
}
