mod m20260328_000001_create_comments_tables;
mod m20260721_000007_expand_comment_locale_storage_columns;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260328_000001_create_comments_tables::Migration),
        Box::new(m20260721_000007_expand_comment_locale_storage_columns::Migration),
    ]
}
