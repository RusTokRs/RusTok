mod m20260723_000001_create_social_graph_relations;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260723_000001_create_social_graph_relations::Migration,
    )]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260723_000001_create_social_graph_relations",
        vec!["m20250101_000002_create_users"],
    )]
}
