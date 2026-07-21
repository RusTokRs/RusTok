mod m20260719_000001_create_marketplace_ledger;
mod m20260721_000002_add_reversals_and_seller_balances;
mod m20260721_000003_add_seller_balance_transfers;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260719_000001_create_marketplace_ledger::Migration),
        Box::new(m20260721_000002_add_reversals_and_seller_balances::Migration),
        Box::new(m20260721_000003_add_seller_balance_transfers::Migration),
    ]
}
