mod m20260325_000102_create_cart_tables;
mod m20260326_000103_add_cart_context_columns;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000102_create_cart_tables::Migration),
        Box::new(m20260326_000103_add_cart_context_columns::Migration),
    ]
}
