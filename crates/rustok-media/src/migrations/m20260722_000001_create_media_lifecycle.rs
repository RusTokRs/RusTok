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
                    .table(Assets::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Assets::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Assets::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Assets::UploadedBy).uuid())
                    .col(ColumnDef::new(Assets::UploadSessionId).uuid())
                    .col(ColumnDef::new(Assets::ActiveBlobId).uuid())
                    .col(
                        ColumnDef::new(Assets::OriginalName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Assets::LifecycleState)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(Assets::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Assets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Assets::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Assets::DeleteRequestedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Assets::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_assets_tenant")
                            .from(Assets::Table, Assets::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_assets_uploader")
                            .from(Assets::Table, Assets::UploadedBy)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_media_assets_tenant_id")
                    .table(Assets::Table)
                    .col(Assets::TenantId)
                    .col(Assets::Id)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Blobs::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Blobs::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Blobs::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Blobs::AssetId).uuid().not_null())
                    .col(ColumnDef::new(Blobs::ObjectKey).string_len(768).not_null())
                    .col(ColumnDef::new(Blobs::MimeType).string_len(100).not_null())
                    .col(ColumnDef::new(Blobs::Size).big_integer().not_null())
                    .col(
                        ColumnDef::new(Blobs::ChecksumSha256)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Blobs::Width).integer())
                    .col(ColumnDef::new(Blobs::Height).integer())
                    .col(ColumnDef::new(Blobs::State).string_len(32).not_null())
                    .col(
                        ColumnDef::new(Blobs::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Blobs::ReadyAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Blobs::DeleteRequestedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(Blobs::DeletedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(Blobs::ReconcileAttempts)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Blobs::LastReconciledAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Blobs::LastError).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_blobs_tenant")
                            .from(Blobs::Table, Blobs::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_blobs_tenant_asset")
                            .from_tbl(Blobs::Table)
                            .from_col(Blobs::TenantId)
                            .from_col(Blobs::AssetId)
                            .to_tbl(Assets::Table)
                            .to_col(Assets::TenantId)
                            .to_col(Assets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_media_blobs_tenant_id")
                    .table(Blobs::Table)
                    .col(Blobs::TenantId)
                    .col(Blobs::Id)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PortOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PortOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PortOperations::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(PortOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortOperations::Operation)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PortOperations::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PortOperations::LeaseToken).uuid().not_null())
                    .col(
                        ColumnDef::new(PortOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PortOperations::ResponseJson).json_binary())
                    .col(ColumnDef::new(PortOperations::ErrorJson).json_binary())
                    .col(
                        ColumnDef::new(PortOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PortOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(PortOperations::CompletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_port_operations_tenant")
                            .from(PortOperations::Table, PortOperations::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uidx_media_port_operations_tenant_key")
                    .table(PortOperations::Table)
                    .col(PortOperations::TenantId)
                    .col(PortOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_media_assets_active_blob")
                        .from(Assets::Table, Assets::ActiveBlobId)
                        .to(Blobs::Table, Blobs::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(Renditions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Renditions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Renditions::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Renditions::AssetId).uuid().not_null())
                    .col(ColumnDef::new(Renditions::SourceBlobId).uuid().not_null())
                    .col(ColumnDef::new(Renditions::ResultBlobId).uuid())
                    .col(
                        ColumnDef::new(Renditions::RecipeHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Renditions::Recipe).json_binary().not_null())
                    .col(
                        ColumnDef::new(Renditions::Purpose)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Renditions::State).string_len(32).not_null())
                    .col(
                        ColumnDef::new(Renditions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Renditions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Renditions::LastError).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_renditions_tenant")
                            .from(Renditions::Table, Renditions::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_renditions_tenant_asset")
                            .from_tbl(Renditions::Table)
                            .from_col(Renditions::TenantId)
                            .from_col(Renditions::AssetId)
                            .to_tbl(Assets::Table)
                            .to_col(Assets::TenantId)
                            .to_col(Assets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_renditions_tenant_source_blob")
                            .from_tbl(Renditions::Table)
                            .from_col(Renditions::TenantId)
                            .from_col(Renditions::SourceBlobId)
                            .to_tbl(Blobs::Table)
                            .to_col(Blobs::TenantId)
                            .to_col(Blobs::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_renditions_tenant_result_blob")
                            .from_tbl(Renditions::Table)
                            .from_col(Renditions::TenantId)
                            .from_col(Renditions::ResultBlobId)
                            .to_tbl(Blobs::Table)
                            .to_col(Blobs::TenantId)
                            .to_col(Blobs::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(UploadSessions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(UploadSessions::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(UploadSessions::TenantId).uuid().not_null())
                    .col(ColumnDef::new(UploadSessions::ActorId).uuid())
                    .col(
                        ColumnDef::new(UploadSessions::StagingKey)
                            .string_len(768)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::OriginalName)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::ExpectedMimeType)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(ColumnDef::new(UploadSessions::ExpectedSize).big_integer())
                    .col(
                        ColumnDef::new(UploadSessions::State)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(UploadSessions::ExpiresAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(UploadSessions::CompletedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(UploadSessions::StagingDeletedAt).timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(UploadSessions::LastError).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_upload_sessions_tenant")
                            .from(UploadSessions::Table, UploadSessions::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_upload_sessions_actor")
                            .from(UploadSessions::Table, UploadSessions::ActorId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .create_foreign_key(
                    ForeignKey::create()
                        .name("fk_media_assets_upload_session")
                        .from(Assets::Table, Assets::UploadSessionId)
                        .to(UploadSessions::Table, UploadSessions::Id)
                        .on_delete(ForeignKeyAction::SetNull)
                        .to_owned(),
                )
                .await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(Translations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(Translations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(Translations::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Translations::AssetId).uuid().not_null())
                    .col(
                        ColumnDef::new(Translations::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(Translations::Title).string_len(255))
                    .col(ColumnDef::new(Translations::AltText).string_len(255))
                    .col(ColumnDef::new(Translations::Caption).text())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_media_translations_tenant_asset")
                            .from_tbl(Translations::Table)
                            .from_col(Translations::TenantId)
                            .from_col(Translations::AssetId)
                            .to_tbl(Assets::Table)
                            .to_col(Assets::TenantId)
                            .to_col(Assets::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        create_indexes(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(PortOperations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Translations::Table).to_owned())
            .await?;
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_media_assets_upload_session")
                        .table(Assets::Table)
                        .to_owned(),
                )
                .await?;
        }
        manager
            .drop_table(Table::drop().table(UploadSessions::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Renditions::Table).to_owned())
            .await?;
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .drop_foreign_key(
                    ForeignKey::drop()
                        .name("fk_media_assets_active_blob")
                        .table(Assets::Table)
                        .to_owned(),
                )
                .await?;
        }
        manager
            .drop_table(Table::drop().table(Blobs::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Assets::Table).to_owned())
            .await
    }
}

async fn create_indexes(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    for index in [
        Index::create()
            .name("idx_media_assets_tenant_state_created")
            .table(Assets::Table)
            .col(Assets::TenantId)
            .col(Assets::LifecycleState)
            .col(Assets::CreatedAt)
            .to_owned(),
        Index::create()
            .name("idx_media_assets_upload_session")
            .table(Assets::Table)
            .col(Assets::UploadSessionId)
            .unique()
            .to_owned(),
        Index::create()
            .name("idx_media_blobs_object_key")
            .table(Blobs::Table)
            .col(Blobs::ObjectKey)
            .unique()
            .to_owned(),
        Index::create()
            .name("idx_media_blobs_asset_state")
            .table(Blobs::Table)
            .col(Blobs::AssetId)
            .col(Blobs::State)
            .to_owned(),
        Index::create()
            .name("idx_media_blobs_tenant_state_reconciled")
            .table(Blobs::Table)
            .col(Blobs::TenantId)
            .col(Blobs::State)
            .col(Blobs::LastReconciledAt)
            .to_owned(),
        Index::create()
            .name("idx_media_renditions_source_recipe")
            .table(Renditions::Table)
            .col(Renditions::SourceBlobId)
            .col(Renditions::RecipeHash)
            .unique()
            .to_owned(),
        Index::create()
            .name("idx_media_upload_sessions_state_expiry")
            .table(UploadSessions::Table)
            .col(UploadSessions::State)
            .col(UploadSessions::ExpiresAt)
            .to_owned(),
        Index::create()
            .name("idx_media_translations_asset_locale")
            .table(Translations::Table)
            .col(Translations::AssetId)
            .col(Translations::Locale)
            .unique()
            .to_owned(),
    ] {
        manager.create_index(index).await?;
    }
    Ok(())
}

#[derive(Iden)]
enum Assets {
    #[iden = "media_assets"]
    Table,
    Id,
    TenantId,
    UploadedBy,
    UploadSessionId,
    ActiveBlobId,
    OriginalName,
    LifecycleState,
    Metadata,
    CreatedAt,
    UpdatedAt,
    DeleteRequestedAt,
    DeletedAt,
}

#[derive(Iden)]
enum Blobs {
    #[iden = "media_blobs"]
    Table,
    Id,
    TenantId,
    AssetId,
    ObjectKey,
    MimeType,
    Size,
    ChecksumSha256,
    Width,
    Height,
    State,
    CreatedAt,
    ReadyAt,
    DeleteRequestedAt,
    DeletedAt,
    ReconcileAttempts,
    LastReconciledAt,
    LastError,
}

#[derive(Iden)]
enum PortOperations {
    #[iden = "media_port_operations"]
    Table,
    Id,
    TenantId,
    IdempotencyKey,
    Operation,
    RequestHash,
    LeaseToken,
    Status,
    ResponseJson,
    ErrorJson,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(Iden)]
enum Renditions {
    #[iden = "media_renditions"]
    Table,
    Id,
    TenantId,
    AssetId,
    SourceBlobId,
    ResultBlobId,
    RecipeHash,
    Recipe,
    Purpose,
    State,
    CreatedAt,
    UpdatedAt,
    LastError,
}

#[derive(Iden)]
enum UploadSessions {
    #[iden = "media_upload_sessions"]
    Table,
    Id,
    TenantId,
    ActorId,
    StagingKey,
    OriginalName,
    ExpectedMimeType,
    ExpectedSize,
    State,
    CreatedAt,
    UpdatedAt,
    ExpiresAt,
    CompletedAt,
    StagingDeletedAt,
    LastError,
}

#[derive(Iden)]
enum Translations {
    #[iden = "media_translations"]
    Table,
    Id,
    TenantId,
    AssetId,
    Locale,
    Title,
    AltText,
    Caption,
}

#[derive(Iden)]
enum Tenants {
    Table,
    Id,
}

#[derive(Iden)]
enum Users {
    Table,
    Id,
}
