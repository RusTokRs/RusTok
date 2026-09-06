use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MarketplaceListingEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplaceListingEvents::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceListingEvents::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceListingEvents::ListingId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceListingEvents::ActorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceListingEvents::EventKind)
                            .string_len(48)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceListingEvents::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MarketplaceListingEvents::Note).text())
                    .col(
                        ColumnDef::new(MarketplaceListingEvents::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(MarketplaceListingEvents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_marketplace_listing_events_tenant_listing")
                            .from(
                                MarketplaceListingEvents::Table,
                                MarketplaceListingEvents::TenantId,
                            )
                            .from_col(MarketplaceListingEvents::ListingId)
                            .to(MarketplaceListings::Table, MarketplaceListings::TenantId)
                            .to_col(MarketplaceListings::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("idx_marketplace_listing_events_timeline")
                .table(MarketplaceListingEvents::Table)
                .col(MarketplaceListingEvents::TenantId)
                .col(MarketplaceListingEvents::ListingId)
                .col(MarketplaceListingEvents::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_listing_events_kind")
                .table(MarketplaceListingEvents::Table)
                .col(MarketplaceListingEvents::TenantId)
                .col(MarketplaceListingEvents::EventKind)
                .col(MarketplaceListingEvents::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_listing_events_actor")
                .table(MarketplaceListingEvents::Table)
                .col(MarketplaceListingEvents::TenantId)
                .col(MarketplaceListingEvents::ActorId)
                .col(MarketplaceListingEvents::CreatedAt)
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
                    .table(MarketplaceListingEvents::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum MarketplaceListings {
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum MarketplaceListingEvents {
    Table,
    Id,
    TenantId,
    ListingId,
    ActorId,
    EventKind,
    Locale,
    Note,
    Metadata,
    CreatedAt,
}
