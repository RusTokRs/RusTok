mod m20260328_000001_create_comments_tables;
mod m20260721_000007_expand_comment_locale_storage_columns;
mod m20260723_000008_cutover_comment_richtext;
mod m20260723_000008_repair_comment_thread_counters;
mod m20260723_000009_add_comment_thread_identity_locks;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260328_000001_create_comments_tables::Migration),
        Box::new(m20260721_000007_expand_comment_locale_storage_columns::Migration),
        Box::new(m20260723_000008_cutover_comment_richtext::Migration),
        Box::new(m20260723_000008_repair_comment_thread_counters::Migration),
        Box::new(m20260723_000009_add_comment_thread_identity_locks::Migration),
    ]
}
