use super::super::*;
use sea_orm_migration::prelude::*;

pub(super) async fn create(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
// 6. Create blog_tag_usage table
        manager
            .create_table(
                Table::create()
                    .table(BlogTagUsage::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(BlogTagUsage::TenantId).uuid().not_null())
                    .col(ColumnDef::new(BlogTagUsage::TagId).uuid().not_null())
                    .col(
                        ColumnDef::new(BlogTagUsage::CanonicalKey)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogTagUsage::UseCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .primary_key(
                        Index::create()
                            .col(BlogTagUsage::TenantId)
                            .col(BlogTagUsage::TagId),
                    )
                    .check(Expr::cust("use_count >= 0"))

                    // Enforce tenant-correct Taxonomy ownership on both PostgreSQL and SQLite.
                    // A dictionary deletion must also delete its Blog attachments.
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_tag_usage_taxonomy_term_tenant")
                            .from_tbl(BlogTagUsage::Table)
                            .from_col(BlogTagUsage::TenantId)
                            .from_col(BlogTagUsage::TagId)
                            .to_tbl(Alias::new("taxonomy_terms"))
                            .to_col(Alias::new("tenant_id"))
                            .to_col(Alias::new("id"))
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_tag_usage_list")
                    .table(BlogTagUsage::Table)
                    .col(BlogTagUsage::TenantId)
                    .col((BlogTagUsage::UseCount, IndexOrder::Desc))
                    .col((BlogTagUsage::CanonicalKey, IndexOrder::Asc))
                    .col((BlogTagUsage::TagId, IndexOrder::Asc))
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
    Ok(())
}
