mod m20260316_000004_create_topic_field_definitions;
mod m20260328_000001_create_forum_tables;
mod m20260329_000001_create_forum_solutions;
mod m20260329_000002_create_forum_votes;
mod m20260329_000003_create_forum_subscriptions;
mod m20260329_000004_create_forum_user_stats;
mod m20260329_000005_create_forum_topic_tags;
mod m20260330_000001_drop_forum_topic_legacy_tags_column;
mod m20260405_000001_add_metadata_to_forum_topics;
mod m20260712_000001_enforce_forum_core_tenant_integrity;
mod m20260712_000002_add_tenant_to_forum_children;
mod m20260712_000003_enforce_forum_relation_tenant_integrity;
mod m20260712_000004_enforce_forum_status_lifecycle;
mod m20260712_000005_enforce_forum_category_tree;
mod m20260712_000006_serialize_forum_counter_mutations;
mod m20260713_000007_enforce_forum_reply_publication;
mod m20260713_000008_enforce_forum_reply_positions;
mod m20260713_000009_add_forum_soft_delete_revisions;
mod m20260713_000010_harden_forum_wave_invariants;
mod m20260713_000011_add_forum_domain_events;
mod m20260713_000012_add_forum_read_model_indexes;
mod m20260713_000013_add_forum_subscription_levels;
mod m20260716_000004_add_topic_field_cache_generation_trigger;
mod m20260721_000001_enforce_forum_category_depth;
mod m20260721_000002_add_forum_category_topic_policy;
mod m20260721_000003_add_forum_category_subtree_lifecycle;
mod m20260722_000004_add_forum_mention_quote_relations;
mod m20260722_000005_seed_forum_relation_revisions;
mod m20260722_000006_add_forum_mention_events;

use rustok_core::MigrationDependencyDescriptor;
use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260316_000004_create_topic_field_definitions::Migration),
        Box::new(m20260328_000001_create_forum_tables::Migration),
        Box::new(m20260329_000001_create_forum_solutions::Migration),
        Box::new(m20260329_000002_create_forum_votes::Migration),
        Box::new(m20260329_000003_create_forum_subscriptions::Migration),
        Box::new(m20260329_000004_create_forum_user_stats::Migration),
        Box::new(m20260329_000005_create_forum_topic_tags::Migration),
        Box::new(m20260330_000001_drop_forum_topic_legacy_tags_column::Migration),
        Box::new(m20260405_000001_add_metadata_to_forum_topics::Migration),
        Box::new(m20260712_000001_enforce_forum_core_tenant_integrity::Migration),
        Box::new(m20260712_000002_add_tenant_to_forum_children::Migration),
        Box::new(m20260712_000003_enforce_forum_relation_tenant_integrity::Migration),
        Box::new(m20260712_000004_enforce_forum_status_lifecycle::Migration),
        Box::new(m20260712_000005_enforce_forum_category_tree::Migration),
        Box::new(m20260712_000006_serialize_forum_counter_mutations::Migration),
        Box::new(m20260713_000007_enforce_forum_reply_publication::Migration),
        Box::new(m20260713_000008_enforce_forum_reply_positions::Migration),
        Box::new(m20260713_000009_add_forum_soft_delete_revisions::Migration),
        Box::new(m20260713_000010_harden_forum_wave_invariants::Migration),
        Box::new(m20260713_000011_add_forum_domain_events::Migration),
        Box::new(m20260713_000012_add_forum_read_model_indexes::Migration),
        Box::new(m20260713_000013_add_forum_subscription_levels::Migration),
        Box::new(m20260716_000004_add_topic_field_cache_generation_trigger::Migration),
        Box::new(m20260721_000001_enforce_forum_category_depth::Migration),
        Box::new(m20260721_000002_add_forum_category_topic_policy::Migration),
        Box::new(m20260721_000003_add_forum_category_subtree_lifecycle::Migration),
        Box::new(m20260722_000004_add_forum_mention_quote_relations::Migration),
        Box::new(m20260722_000005_seed_forum_relation_revisions::Migration),
        Box::new(m20260722_000006_add_forum_mention_events::Migration),
    ]
}

pub fn migration_dependencies() -> Vec<MigrationDependencyDescriptor> {
    vec![
        MigrationDependencyDescriptor::new(
            "m20260329_000005_create_forum_topic_tags",
            vec!["m20260329_000001_create_taxonomy_tables"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260712_000003_enforce_forum_relation_tenant_integrity",
            vec!["m20260711_000001_add_tenant_identity_key"],
        ),
    ]
}
