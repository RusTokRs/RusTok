use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Create builds table
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
                    .col(ColumnDef::new(Builds::ReleaseId).string_len(64))
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

        // Create index on manifest_hash for deduplication
        manager
            .create_index(
                Index::create()
                    .name("idx_builds_manifest_hash")
                    .table(Builds::Table)
                    .col(Builds::ManifestHash)
                    .to_owned(),
            )
            .await?;

        // Create index on status for querying
        manager
            .create_index(
                Index::create()
                    .name("idx_builds_status")
                    .table(Builds::Table)
                    .col(Builds::Status)
                    .to_owned(),
            )
            .await?;

        // Create releases table
        manager
            .create_table(
                Table::create()
                    .table(Releases::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Releases::Id)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(Releases::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(Releases::BuildId).uuid().not_null())
                    .col(
                        ColumnDef::new(Releases::Environment)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Releases::ContainerImage).text())
                    .col(ColumnDef::new(Releases::ServerArtifactUrl).text())
                    .col(ColumnDef::new(Releases::AdminArtifactUrl).text())
                    .col(ColumnDef::new(Releases::StorefrontArtifactUrl).text())
                    .col(
                        ColumnDef::new(Releases::ManifestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Releases::Modules).json_binary().not_null())
                    .col(ColumnDef::new(Releases::PreviousReleaseId).string_len(64))
                    .col(ColumnDef::new(Releases::DeployedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Releases::RolledBackAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Releases::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Releases::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        // Create index on status for finding active release
        manager
            .create_index(
                Index::create()
                    .name("idx_releases_status")
                    .table(Releases::Table)
                    .col(Releases::Status)
                    .to_owned(),
            )
            .await?;

        // Create index on build_id
        manager
            .create_index(
                Index::create()
                    .name("idx_releases_build_id")
                    .table(Releases::Table)
                    .col(Releases::BuildId)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Releases::Table).to_owned())
            .await?;

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
    ReleaseId,
    LogsUrl,
    ErrorMessage,
    StartedAt,
    FinishedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
pub enum Releases {
    Table,
    Id,
    Status,
    BuildId,
    Environment,
    ContainerImage,
    ServerArtifactUrl,
    AdminArtifactUrl,
    StorefrontArtifactUrl,
    ManifestHash,
    Modules,
    PreviousReleaseId,
    DeployedAt,
    RolledBackAt,
    CreatedAt,
    UpdatedAt,
}
