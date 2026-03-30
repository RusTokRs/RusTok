mod m20260326_000001_create_profiles_tables;
mod m20260330_000002_create_profile_tags;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260326_000001_create_profiles_tables::Migration),
        Box::new(m20260330_000002_create_profile_tags::Migration),
    ]
}
