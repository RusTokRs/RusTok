mod m20250130_000014_create_regions;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20250130_000014_create_regions::Migration)]
}
