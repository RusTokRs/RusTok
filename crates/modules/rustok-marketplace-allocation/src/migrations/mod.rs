mod m20260718_000001_create_marketplace_order_allocations;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260718_000001_create_marketplace_order_allocations::Migration,
    )]
}
