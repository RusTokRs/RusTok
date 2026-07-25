mod m20260723_000001_create_social_graph_relations;
mod m20260725_000002_add_follow_relation_kind;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260723_000001_create_social_graph_relations::Migration),
        Box::new(m20260725_000002_add_follow_relation_kind::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260723_000001_create_social_graph_relations",
            vec!["m20250101_000002_create_users"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260725_000002_add_follow_relation_kind",
            vec!["m20260723_000001_create_social_graph_relations"],
        ),
    ]
}
