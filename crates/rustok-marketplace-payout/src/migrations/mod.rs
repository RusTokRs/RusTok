mod m20260719_000001_create_marketplace_payouts;
mod m20260722_000002_create_marketplace_payout_operations;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260719_000001_create_marketplace_payouts::Migration),
        Box::new(m20260722_000002_create_marketplace_payout_operations::Migration),
    ]
}
