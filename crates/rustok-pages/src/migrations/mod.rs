mod m20260328_000001_create_pages_tables;
mod m20260329_000001_create_page_channel_visibility_table;
mod m20260713_000001_enforce_pages_ordering_uniqueness;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260328_000001_create_pages_tables::Migration),
        Box::new(m20260329_000001_create_page_channel_visibility_table::Migration),
        Box::new(m20260713_000001_enforce_pages_ordering_uniqueness::Migration),
    ]
}
