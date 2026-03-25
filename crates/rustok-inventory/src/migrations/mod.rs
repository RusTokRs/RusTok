mod shared;

mod m20250130_000016_create_commerce_inventory;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20250130_000016_create_commerce_inventory::Migration,
    )]
}
