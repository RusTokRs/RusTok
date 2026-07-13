mod m20260325_000103_create_customers_table;
mod m20260713_000104_enforce_customer_identity;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000103_create_customers_table::Migration),
        Box::new(m20260713_000104_enforce_customer_identity::Migration),
    ]
}
