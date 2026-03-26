mod m20260326_000001_create_profiles_tables;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260326_000001_create_profiles_tables::Migration)]
}
