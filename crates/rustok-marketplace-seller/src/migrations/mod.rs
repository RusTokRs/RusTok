mod m20260716_000001_create_marketplace_sellers;
mod m20260716_000002_create_seller_command_receipts;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260716_000001_create_marketplace_sellers::Migration),
        Box::new(m20260716_000002_create_seller_command_receipts::Migration),
    ]
}
