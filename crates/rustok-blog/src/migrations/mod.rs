mod m20260328_000001_create_blog_post_tables;
mod m20260328_000002_create_blog_taxonomy_tables;
mod m20260329_000001_create_blog_post_channel_visibility_table;
mod m20260716_000001_create_blog_comment_projection_deliveries;
mod m20260721_000005_expand_blog_locale_storage_columns;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260328_000001_create_blog_post_tables::Migration),
        Box::new(m20260328_000002_create_blog_taxonomy_tables::Migration),
        Box::new(m20260329_000001_create_blog_post_channel_visibility_table::Migration),
        Box::new(m20260716_000001_create_blog_comment_projection_deliveries::Migration),
        Box::new(m20260721_000005_expand_blog_locale_storage_columns::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![MigrationDependencyDescriptor::new(
        "m20260328_000002_create_blog_taxonomy_tables",
        vec!["m20260329_000001_create_taxonomy_tables"],
    )]
}
