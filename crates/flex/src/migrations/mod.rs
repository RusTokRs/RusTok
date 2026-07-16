mod m20260716_000000_create_field_definition_cache_generation;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260716_000000_create_field_definition_cache_generation::Migration,
    )]
}
