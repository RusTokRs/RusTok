use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. docs_pages
        manager
            .create_table(
                Table::create()
                    .table(DocsPages::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DocsPages::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DocsPages::TenantId).uuid().not_null())
                    .col(ColumnDef::new(DocsPages::ParentId).uuid())
                    .col(ColumnDef::new(DocsPages::Slug).string_len(255).not_null())
                    .col(ColumnDef::new(DocsPages::Title).string_len(255).not_null())
                    .col(ColumnDef::new(DocsPages::Icon).string_len(64))
                    .col(ColumnDef::new(DocsPages::CoverImage).text())
                    .col(
                        ColumnDef::new(DocsPages::IsArchived)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(DocsPages::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(DocsPages::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(DocsPages::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_docs_pages_parent_id")
                            .from(DocsPages::Table, DocsPages::ParentId)
                            .to(DocsPages::Table, DocsPages::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_docs_pages_tenant_slug")
                    .table(DocsPages::Table)
                    .col(DocsPages::TenantId)
                    .col(DocsPages::Slug)
                    .unique()
                    .and_where(Expr::col((DocsPages::Table, DocsPages::DeletedAt)).is_null())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_docs_pages_tenant_parent")
                    .table(DocsPages::Table)
                    .col(DocsPages::TenantId)
                    .col(DocsPages::ParentId)
                    .to_owned(),
            )
            .await?;

        // 2. docs_snapshots
        manager
            .create_table(
                Table::create()
                    .table(DocsSnapshots::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DocsSnapshots::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DocsSnapshots::TenantId).uuid().not_null())
                    .col(ColumnDef::new(DocsSnapshots::PageId).uuid().not_null())
                    .col(ColumnDef::new(DocsSnapshots::SeqId).big_integer().not_null())
                    .col(ColumnDef::new(DocsSnapshots::SnapshotData).binary().not_null())
                    .col(ColumnDef::new(DocsSnapshots::Frontiers).text().not_null())
                    .col(
                        ColumnDef::new(DocsSnapshots::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_docs_snapshots_page_id")
                            .from(DocsSnapshots::Table, DocsSnapshots::PageId)
                            .to(DocsPages::Table, DocsPages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_docs_snapshots_tenant_page_seq")
                    .table(DocsSnapshots::Table)
                    .col(DocsSnapshots::TenantId)
                    .col(DocsSnapshots::PageId)
                    .col(DocsSnapshots::SeqId)
                    .to_owned(),
            )
            .await?;

        // 3. docs_updates
        manager
            .create_table(
                Table::create()
                    .table(DocsUpdates::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DocsUpdates::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DocsUpdates::TenantId).uuid().not_null())
                    .col(ColumnDef::new(DocsUpdates::PageId).uuid().not_null())
                    .col(ColumnDef::new(DocsUpdates::SeqId).big_integer().not_null())
                    .col(ColumnDef::new(DocsUpdates::ChangeData).binary().not_null())
                    .col(
                        ColumnDef::new(DocsUpdates::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_docs_updates_page_id")
                            .from(DocsUpdates::Table, DocsUpdates::PageId)
                            .to(DocsPages::Table, DocsPages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_docs_updates_page_seq")
                    .table(DocsUpdates::Table)
                    .col(DocsUpdates::PageId)
                    .col(DocsUpdates::SeqId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_docs_updates_tenant_page")
                    .table(DocsUpdates::Table)
                    .col(DocsUpdates::TenantId)
                    .col(DocsUpdates::PageId)
                    .to_owned(),
            )
            .await?;

        // 4. docs_entity_links (UER backlinks registry)
        manager
            .create_table(
                Table::create()
                    .table(DocsEntityLinks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(DocsEntityLinks::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(DocsEntityLinks::TenantId).uuid().not_null())
                    .col(ColumnDef::new(DocsEntityLinks::SourcePageId).uuid().not_null())
                    .col(ColumnDef::new(DocsEntityLinks::TargetModule).string_len(64).not_null())
                    .col(ColumnDef::new(DocsEntityLinks::TargetEntityType).string_len(64).not_null())
                    .col(ColumnDef::new(DocsEntityLinks::TargetEntityId).string_len(255).not_null())
                    .col(
                        ColumnDef::new(DocsEntityLinks::DisplayMode)
                            .string_len(32)
                            .not_null()
                            .default("embed"),
                    )
                    .col(
                        ColumnDef::new(DocsEntityLinks::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_docs_entity_links_source_page_id")
                            .from(DocsEntityLinks::Table, DocsEntityLinks::SourcePageId)
                            .to(DocsPages::Table, DocsPages::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_docs_entity_links_target")
                    .table(DocsEntityLinks::Table)
                    .col(DocsEntityLinks::TenantId)
                    .col(DocsEntityLinks::TargetModule)
                    .col(DocsEntityLinks::TargetEntityType)
                    .col(DocsEntityLinks::TargetEntityId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_docs_entity_links_source")
                    .table(DocsEntityLinks::Table)
                    .col(DocsEntityLinks::TenantId)
                    .col(DocsEntityLinks::SourcePageId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(DocsEntityLinks::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DocsUpdates::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DocsSnapshots::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(DocsPages::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum DocsPages {
    Table,
    Id,
    TenantId,
    ParentId,
    Slug,
    Title,
    Icon,
    CoverImage,
    IsArchived,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(DeriveIden)]
enum DocsSnapshots {
    Table,
    Id,
    TenantId,
    PageId,
    SeqId,
    SnapshotData,
    Frontiers,
    CreatedAt,
}

#[derive(DeriveIden)]
enum DocsUpdates {
    Table,
    Id,
    TenantId,
    PageId,
    SeqId,
    ChangeData,
    CreatedAt,
}

#[derive(DeriveIden)]
enum DocsEntityLinks {
    Table,
    Id,
    TenantId,
    SourcePageId,
    TargetModule,
    TargetEntityType,
    TargetEntityId,
    DisplayMode,
    CreatedAt,
}
