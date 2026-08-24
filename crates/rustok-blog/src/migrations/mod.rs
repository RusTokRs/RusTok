mod m20260328_000001_create_blog_post_tables;
mod m20260328_000002_create_blog_taxonomy_tables;
mod m20260329_000001_create_blog_post_channel_visibility_table;
mod m20260716_000001_create_blog_comment_projection_deliveries;
mod m20260721_000005_expand_blog_locale_storage_columns;
mod m20260801_000007_create_blog_comments_delegation_schedule_state;
mod m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox;
mod m20260803_000009_add_blog_comments_audit_canonical_handoff;
mod m20260803_000010_add_blog_comments_audit_source_retry_policy;
mod m20260803_000011_create_blog_comments_audit_recovery;
mod m20260803_000016_add_blog_category_translation_target_support;
mod m20260812_000017_enforce_blog_category_hierarchy;
mod m20260813_000018_enforce_blog_post_tag_tenant_integrity;
mod m20260824_000019_add_blog_taxonomy_category_binding;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260328_000001_create_blog_post_tables::Migration),
        Box::new(m20260328_000002_create_blog_taxonomy_tables::Migration),
        Box::new(m20260329_000001_create_blog_post_channel_visibility_table::Migration),
        Box::new(m20260716_000001_create_blog_comment_projection_deliveries::Migration),
        Box::new(m20260721_000005_expand_blog_locale_storage_columns::Migration),
        Box::new(m20260801_000007_create_blog_comments_delegation_schedule_state::Migration),
        Box::new(m20260801_000008_create_blog_comments_delegation_schedule_audit_outbox::Migration),
        Box::new(m20260803_000009_add_blog_comments_audit_canonical_handoff::Migration),
        Box::new(m20260803_000010_add_blog_comments_audit_source_retry_policy::Migration),
        Box::new(m20260803_000011_create_blog_comments_audit_recovery::Migration),
        Box::new(m20260803_000016_add_blog_category_translation_target_support::Migration),
        Box::new(m20260812_000017_enforce_blog_category_hierarchy::Migration),
        Box::new(m20260813_000018_enforce_blog_post_tag_tenant_integrity::Migration),
        Box::new(m20260824_000019_add_blog_taxonomy_category_binding::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260328_000002_create_blog_taxonomy_tables",
            vec!["m20260329_000001_create_taxonomy_tables"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260803_000016_add_blog_category_translation_target_support",
            vec!["m20260803_000001_create_owner_operation_receipts"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260813_000018_enforce_blog_post_tag_tenant_integrity",
            vec!["m20260711_000001_add_tenant_identity_key"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260824_000019_add_blog_taxonomy_category_binding",
            vec![
                "m20260711_000001_add_tenant_identity_key",
                "m20260812_000017_enforce_blog_category_hierarchy",
            ],
        ),
    ]
}
