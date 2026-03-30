mod m20260316_000004_create_topic_field_definitions;
mod m20260328_000001_create_forum_tables;
mod m20260329_000001_create_forum_solutions;
mod m20260329_000002_create_forum_votes;
mod m20260329_000003_create_forum_subscriptions;
mod m20260329_000004_create_forum_user_stats;
mod m20260329_000005_create_forum_topic_tags;
mod m20260330_000001_drop_forum_topic_legacy_tags_column;

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
    ]
}
