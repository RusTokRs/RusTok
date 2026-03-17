mod shared;

mod m20250130_000010_create_index_content;
mod m20250130_000010a_create_search_index;
mod m20250130_000011_create_index_products;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20250130_000010_create_index_content::Migration),
        Box::new(m20250130_000010a_create_search_index::Migration),
        Box::new(m20250130_000011_create_index_products::Migration),
    ]
}
