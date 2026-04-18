mod m20260325_000104_create_payment_tables;
mod m20260416_000105_create_refunds_table;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000104_create_payment_tables::Migration),
        Box::new(m20260416_000105_create_refunds_table::Migration),
    ]
}
