mod m20260718_000001_create_marketplace_commission;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260718_000001_create_marketplace_commission::Migration,
    )]
}
