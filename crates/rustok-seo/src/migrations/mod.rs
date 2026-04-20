mod m20260419_000002_expand_seo_locale_columns;
mod m20260419_000003_create_seo_tables;

use sea_orm_migration::prelude::*;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260419_000002_expand_seo_locale_columns::Migration),
        Box::new(m20260419_000003_create_seo_tables::Migration),
    ]
}
