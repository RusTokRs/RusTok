use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MarketplaceSellerEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplaceSellerEvents::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerEvents::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerEvents::SellerId)
                            .uuid()
                            .not_null(),
                    )
                    .col(ColumnDef::new(MarketplaceSellerEvents::ActorId).uuid())
                    .col(
                        ColumnDef::new(MarketplaceSellerEvents::EventKind)
                            .string_len(48)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MarketplaceSellerEvents::Locale).string_len(32))
                    .col(
                        ColumnDef::new(MarketplaceSellerEvents::Provenance)
                            .string_len(32)
                            .not_null()
                            .default("command"),
                    )
                    .col(ColumnDef::new(MarketplaceSellerEvents::Note).text())
                    .col(
                        ColumnDef::new(MarketplaceSellerEvents::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerEvents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_marketplace_seller_events_tenant_seller")
                            .from(
                                MarketplaceSellerEvents::Table,
                                MarketplaceSellerEvents::TenantId,
                            )
                            .from_col(MarketplaceSellerEvents::SellerId)
                            .to(MarketplaceSellers::Table, MarketplaceSellers::TenantId)
                            .to_col(MarketplaceSellers::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust(
                        "(provenance = 'command' AND actor_id IS NOT NULL AND locale IS NOT NULL) OR (provenance = 'legacy_snapshot' AND actor_id IS NULL AND locale IS NULL)",
                    ))
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("idx_marketplace_seller_events_timeline")
                .table(MarketplaceSellerEvents::Table)
                .col(MarketplaceSellerEvents::TenantId)
                .col(MarketplaceSellerEvents::SellerId)
                .col(MarketplaceSellerEvents::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_seller_events_kind")
                .table(MarketplaceSellerEvents::Table)
                .col(MarketplaceSellerEvents::TenantId)
                .col(MarketplaceSellerEvents::EventKind)
                .col(MarketplaceSellerEvents::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_seller_events_actor")
                .table(MarketplaceSellerEvents::Table)
                .col(MarketplaceSellerEvents::TenantId)
                .col(MarketplaceSellerEvents::ActorId)
                .col(MarketplaceSellerEvents::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplaceSellerEvents::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum MarketplaceSellers {
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum MarketplaceSellerEvents {
    Table,
    Id,
    TenantId,
    SellerId,
    ActorId,
    EventKind,
    Locale,
    Provenance,
    Note,
    Metadata,
    CreatedAt,
}
