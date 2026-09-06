mod m20260719_000001_create_marketplace_payouts;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260719_000001_create_marketplace_payouts::Migration,
    )]
}
