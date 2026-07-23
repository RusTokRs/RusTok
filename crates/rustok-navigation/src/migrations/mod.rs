mod m20260722_000001_create_navigation_tables;
use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;
pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260722_000001_create_navigation_tables::Migration,
    )]
}
pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260722_000001_create_navigation_tables",
        vec!["m20260325_000001_create_channels"],
    )]
}
