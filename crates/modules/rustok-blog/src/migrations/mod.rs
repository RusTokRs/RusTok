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
mod m20260824_000020_backfill_blog_categories_to_taxonomy;
mod m20260828_000021_retire_blog_category_legacy_storage;
mod m20260916_000022_clean_blog_category_canonical_taxonomy;
mod m20260919_000023_enforce_blog_post_category_tenant_integrity;
mod m20260919_000024_enforce_blog_post_channel_visibility_tenant_integrity;
mod m20260919_000025_fix_blog_post_category_tenant_delete_action;
mod m20260922_000026_add_blog_comment_projection_revision;
mod m20260922_000027_create_blog_tag_usage_projection;
mod m20260922_000028_remove_blog_category_post_count;

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
        Box::new(m20260824_000020_backfill_blog_categories_to_taxonomy::Migration),
        Box::new(m20260828_000021_retire_blog_category_legacy_storage::Migration),
        Box::new(m20260916_000022_clean_blog_category_canonical_taxonomy::Migration),
        Box::new(m20260919_000023_enforce_blog_post_category_tenant_integrity::Migration),
        Box::new(m20260919_000024_enforce_blog_post_channel_visibility_tenant_integrity::Migration),
        Box::new(m20260919_000025_fix_blog_post_category_tenant_delete_action::Migration),
        Box::new(m20260922_000026_add_blog_comment_projection_revision::Migration),
        Box::new(m20260922_000027_create_blog_tag_usage_projection::Migration),
        Box::new(m20260922_000028_remove_blog_category_post_count::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260922_000026_add_blog_comment_projection_revision",
            vec!["m20260716_000001_create_blog_comment_projection_deliveries"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260922_000028_remove_blog_category_post_count",
            vec!["m20260916_000022_clean_blog_category_canonical_taxonomy"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260922_000027_create_blog_tag_usage_projection",
            vec![
                "m20260813_000018_enforce_blog_post_tag_tenant_integrity",
                "m20260711_000001_add_tenant_identity_key",
            ],
        ),
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
        MigrationDependencyDescriptor::new(
            "m20260824_000020_backfill_blog_categories_to_taxonomy",
            vec![
                "m20260822_000010_create_taxonomy_category_hierarchy",
                "m20260824_000019_add_blog_taxonomy_category_binding",
            ],
        ),
        MigrationDependencyDescriptor::new(
            "m20260828_000021_retire_blog_category_legacy_storage",
            vec!["m20260824_000020_backfill_blog_categories_to_taxonomy"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260916_000022_clean_blog_category_canonical_taxonomy",
            vec!["m20260828_000021_retire_blog_category_legacy_storage"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260919_000023_enforce_blog_post_category_tenant_integrity",
            vec!["m20260916_000022_clean_blog_category_canonical_taxonomy"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260919_000024_enforce_blog_post_channel_visibility_tenant_integrity",
            vec!["m20260919_000023_enforce_blog_post_category_tenant_integrity"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260919_000025_fix_blog_post_category_tenant_delete_action",
            vec!["m20260919_000023_enforce_blog_post_category_tenant_integrity"],
        ),
    ]
}
