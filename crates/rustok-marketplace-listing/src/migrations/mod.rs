mod m20260716_000001_create_marketplace_listings;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260716_000001_create_marketplace_listings::Migration,
    )]
}
