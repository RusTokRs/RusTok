mod m20260325_000104_create_payment_tables;
mod m20260416_000105_create_refunds_table;
mod m20260713_000106_enforce_payment_lifecycle_uniqueness;
mod m20260713_000107_enforce_refund_capacity;
mod m20260713_000108_enforce_payment_state_invariants;
mod m20260713_000109_serialize_payment_lifecycle;
mod m20260713_000110_create_provider_operation_journal;
mod m20260713_000111_enforce_provider_operation_lifecycle;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000104_create_payment_tables::Migration),
        Box::new(m20260416_000105_create_refunds_table::Migration),
        Box::new(m20260713_000106_enforce_payment_lifecycle_uniqueness::Migration),
        Box::new(m20260713_000107_enforce_refund_capacity::Migration),
        Box::new(m20260713_000108_enforce_payment_state_invariants::Migration),
        Box::new(m20260713_000109_serialize_payment_lifecycle::Migration),
        Box::new(m20260713_000110_create_provider_operation_journal::Migration),
        Box::new(m20260713_000111_enforce_provider_operation_lifecycle::Migration),
    ]
}
