mod m20260716_000001_create_marketplace_listings;
mod m20260717_000002_create_marketplace_listing_events;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260716_000001_create_marketplace_listings::Migration),
        Box::new(m20260717_000002_create_marketplace_listing_events::Migration),
    ]
}
