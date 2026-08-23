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
mod m20260724_000001_add_forum_topic_read_states;
mod m20260724_000002_add_forum_category_visibility_policy;
mod m20260725_000001_add_forum_category_audience_policy;
mod m20260725_000002_add_forum_topic_audience_policy;
mod m20260727_000001_add_forum_category_topic_create_audience;
mod m20260728_000001_add_forum_category_reply_create_audience;
mod m20260728_000002_add_forum_topic_reply_create_audience;
mod m20260728_000003_add_forum_category_moderation_audience;
mod m20260728_000004_add_forum_user_trust_state;
mod m20260728_000005_add_forum_approved_posts_indexes;
mod m20260728_000006_add_forum_create_window_indexes;
mod m20260731_000007_add_forum_projection_revision_ledger;
mod m20260731_000008_harden_forum_projection_revision_counter;
mod m20260801_000009_add_forum_topic_move_operations;
mod m20260801_000010_add_forum_topic_merge_operations;
mod m20260801_000011_add_forum_topic_merge_subscription_reconciliations;
mod m20260801_000012_add_forum_topic_merge_read_state_reconciliations;
mod m20260803_000013_add_forum_topic_merge_tag_reconciliations;
mod m20260803_000014_add_forum_topic_merge_vote_reconciliations;
mod m20260803_000015_add_forum_topic_merge_audience_reconciliations;
mod m20260803_000016_add_forum_topic_merge_solution_policy;
mod m20260803_000017_add_forum_topic_canonical_resolution;
mod m20260803_000018_add_forum_topic_merge_solution_resolution;
mod m20260803_000019_allow_cross_category_topic_merge_redirect_edges;
mod m20260803_000020_add_forum_topic_split_operations;
mod m20260804_000021_add_forum_topic_fork_operations;
mod m20260804_000022_add_forum_reply_range_move_operations;
mod m20260804_000023_advance_forum_reply_range_move_positions;
mod m20260805_000024_add_forum_topic_route_aliases;
mod m20260806_000025_add_forum_topic_route_tombstone_visibility;
mod m20260806_000026_add_forum_category_route_aliases;
mod m20260807_000027_add_forum_moderation_subject_revisions;
mod m20260820_000028_add_forum_category_translation_changes;
mod m20260823_000029_add_forum_taxonomy_category_binding;

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
        Box::new(m20260724_000001_add_forum_topic_read_states::Migration),
        Box::new(m20260724_000002_add_forum_category_visibility_policy::Migration),
        Box::new(m20260725_000001_add_forum_category_audience_policy::Migration),
        Box::new(m20260725_000002_add_forum_topic_audience_policy::Migration),
        Box::new(m20260727_000001_add_forum_category_topic_create_audience::Migration),
        Box::new(m20260728_000001_add_forum_category_reply_create_audience::Migration),
        Box::new(m20260728_000002_add_forum_topic_reply_create_audience::Migration),
        Box::new(m20260728_000003_add_forum_category_moderation_audience::Migration),
        Box::new(m20260728_000004_add_forum_user_trust_state::Migration),
        Box::new(m20260728_000005_add_forum_approved_posts_indexes::Migration),
        Box::new(m20260728_000006_add_forum_create_window_indexes::Migration),
        Box::new(m20260731_000007_add_forum_projection_revision_ledger::Migration),
        Box::new(m20260731_000008_harden_forum_projection_revision_counter::Migration),
        Box::new(m20260801_000009_add_forum_topic_move_operations::Migration),
        Box::new(m20260801_000010_add_forum_topic_merge_operations::Migration),
        Box::new(m20260801_000011_add_forum_topic_merge_subscription_reconciliations::Migration),
        Box::new(m20260801_000012_add_forum_topic_merge_read_state_reconciliations::Migration),
        Box::new(m20260803_000013_add_forum_topic_merge_tag_reconciliations::Migration),
        Box::new(m20260803_000014_add_forum_topic_merge_vote_reconciliations::Migration),
        Box::new(m20260803_000015_add_forum_topic_merge_audience_reconciliations::Migration),
        Box::new(m20260803_000016_add_forum_topic_merge_solution_policy::Migration),
        Box::new(m20260803_000017_add_forum_topic_canonical_resolution::Migration),
        Box::new(m20260803_000018_add_forum_topic_merge_solution_resolution::Migration),
        Box::new(m20260803_000019_allow_cross_category_topic_merge_redirect_edges::Migration),
        Box::new(m20260803_000020_add_forum_topic_split_operations::Migration),
        Box::new(m20260804_000021_add_forum_topic_fork_operations::Migration),
        Box::new(m20260804_000022_add_forum_reply_range_move_operations::Migration),
        Box::new(m20260804_000023_advance_forum_reply_range_move_positions::Migration),
        Box::new(m20260805_000024_add_forum_topic_route_aliases::Migration),
        Box::new(m20260806_000025_add_forum_topic_route_tombstone_visibility::Migration),
        Box::new(m20260806_000026_add_forum_category_route_aliases::Migration),
        Box::new(m20260807_000027_add_forum_moderation_subject_revisions::Migration),
        Box::new(m20260820_000028_add_forum_category_translation_changes::Migration),
        Box::new(m20260823_000029_add_forum_taxonomy_category_binding::Migration),
    ]
}

#[cfg(test)]
pub(crate) fn relation_migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260722_000004_add_forum_mention_quote_relations::Migration,
    )]
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
        MigrationDependencyDescriptor::new(
            "m20260716_000004_add_topic_field_cache_generation_trigger",
            vec!["m20260716_000000_create_field_definition_cache_generation"],
        ),
        MigrationDependencyDescriptor::new(
            "m20260823_000029_add_forum_taxonomy_category_binding",
            vec!["m20260711_000001_add_tenant_identity_key"],
        ),
    ]
}
