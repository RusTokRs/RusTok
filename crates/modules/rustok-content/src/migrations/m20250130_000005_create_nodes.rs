use super::shared::*;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Nodes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Nodes::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Nodes::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Nodes::ParentId).uuid())
                    .col(ColumnDef::new(Nodes::AuthorId).uuid())
                    .col(ColumnDef::new(Nodes::Kind).string_len(32).not_null())
                    .col(ColumnDef::new(Nodes::CategoryId).uuid())
                    .col(
                        ColumnDef::new(Nodes::Status)
                            .string_len(32)
                            .not_null()
                            .default("draft"),
                    )
                    .col(
                        ColumnDef::new(Nodes::Position)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(Nodes::Depth).integer().not_null().default(0))
                    .col(
                        ColumnDef::new(Nodes::ReplyCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(Nodes::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(Nodes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Nodes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Nodes::PublishedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .from(Nodes::Table, Nodes::TenantId)
                            .to(Tenants::Table, Tenants::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Nodes::Table, Nodes::ParentId)
                            .to(Nodes::Table, Nodes::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Nodes::Table, Nodes::AuthorId)
                            .to(Users::Table, Users::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(NodeTranslations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(NodeTranslations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(NodeTranslations::NodeId).uuid().not_null())
                    .col(
                        ColumnDef::new(NodeTranslations::Locale)
                            .string_len(5)
                            .not_null(),
                    )
                    .col(ColumnDef::new(NodeTranslations::Title).string_len(255))
                    .col(ColumnDef::new(NodeTranslations::Slug).string_len(255))
                    .col(ColumnDef::new(NodeTranslations::Excerpt).text())
                    .col(
                        ColumnDef::new(NodeTranslations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(NodeTranslations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(NodeTranslations::Table, NodeTranslations::NodeId)
                            .to(Nodes::Table, Nodes::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Bodies::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Bodies::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Bodies::NodeId).uuid().not_null())
                    .col(ColumnDef::new(Bodies::Locale).string_len(5).not_null())
                    .col(ColumnDef::new(Bodies::Body).text())
                    .col(
                        ColumnDef::new(Bodies::Format)
                            .string_len(16)
                            .not_null()
                            .default("markdown"),
                    )
                    .col(
                        ColumnDef::new(Bodies::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(Bodies::Table, Bodies::NodeId)
                            .to(Nodes::Table, Nodes::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_nodes_tenant_kind")
                    .table(Nodes::Table)
                    .col(Nodes::TenantId)
                    .col(Nodes::Kind)
                    .col(Nodes::Status)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_nodes_parent")
                    .table(Nodes::Table)
                    .col(Nodes::ParentId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_nodes_author")
                    .table(Nodes::Table)
                    .col(Nodes::AuthorId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_nodes_category")
                    .table(Nodes::Table)
                    .col(Nodes::CategoryId)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_nodes_published")
                    .table(Nodes::Table)
                    .col(Nodes::TenantId)
                    .col(Nodes::Kind)
                    .col(Nodes::PublishedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_node_trans_unique")
                    .table(NodeTranslations::Table)
                    .col(NodeTranslations::NodeId)
                    .col(NodeTranslations::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_node_trans_slug")
                    .table(NodeTranslations::Table)
                    .col(NodeTranslations::Locale)
                    .col(NodeTranslations::Slug)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_bodies_unique")
                    .table(Bodies::Table)
                    .col(Bodies::NodeId)
                    .col(Bodies::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Bodies::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(NodeTranslations::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Nodes::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum Nodes {
    Table,
    Id,
    TenantId,
    ParentId,
    AuthorId,
    Kind,
    CategoryId,
    Status,
    Position,
    Depth,
    ReplyCount,
    Metadata,
    CreatedAt,
    UpdatedAt,
    PublishedAt,
}

#[derive(Iden)]
enum NodeTranslations {
    Table,
    Id,
    NodeId,
    Locale,
    Title,
    Slug,
    Excerpt,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum Bodies {
    Table,
    Id,
    NodeId,
    Locale,
    Body,
    Format,
    UpdatedAt,
}
