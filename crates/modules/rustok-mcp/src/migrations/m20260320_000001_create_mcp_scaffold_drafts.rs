use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(McpScaffoldDrafts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(McpScaffoldDrafts::ClientId).uuid().null())
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::Slug)
                            .string_len(96)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::CrateName)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::RequestPayload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::PreviewPayload)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::WorkspaceRoot)
                            .text()
                            .null(),
                    )
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::AppliedAt)
                            .timestamp_with_time_zone()
                            .null(),
                    )
                    .col(ColumnDef::new(McpScaffoldDrafts::CreatedBy).uuid().null())
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(McpScaffoldDrafts::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(McpScaffoldDrafts::Table, McpScaffoldDrafts::TenantId)
                            .to(Alias::new("tenants"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(McpScaffoldDrafts::Table, McpScaffoldDrafts::ClientId)
                            .to(Alias::new("mcp_clients"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(McpScaffoldDrafts::Table, McpScaffoldDrafts::CreatedBy)
                            .to(Alias::new("users"), Alias::new("id"))
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_mcp_scaffold_drafts_tenant_created")
                    .table(McpScaffoldDrafts::Table)
                    .col(McpScaffoldDrafts::TenantId)
                    .col(McpScaffoldDrafts::CreatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_mcp_scaffold_drafts_tenant_status")
                    .table(McpScaffoldDrafts::Table)
                    .col(McpScaffoldDrafts::TenantId)
                    .col(McpScaffoldDrafts::Status)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(McpScaffoldDrafts::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum McpScaffoldDrafts {
    Table,
    Id,
    TenantId,
    ClientId,
    Slug,
    CrateName,
    Status,
    RequestPayload,
    PreviewPayload,
    WorkspaceRoot,
    AppliedAt,
    CreatedBy,
    CreatedAt,
    UpdatedAt,
}
