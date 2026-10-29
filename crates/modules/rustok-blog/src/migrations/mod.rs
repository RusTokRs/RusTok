pub mod m20261029_000001_create_blog_schema;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20261029_000001_create_blog_schema::Migration)]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20261029_000001_create_blog_schema",
        vec![
            "m20260711_000001_add_tenant_identity_key",
            "m20260329_000001_create_taxonomy_tables",
        ],
    )]
}
