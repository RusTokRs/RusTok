mod m20260419_000002_expand_seo_locale_columns;
mod m20260419_000003_create_seo_tables;
mod m20260420_000004_create_seo_bulk_tables;
mod m20260421_000005_create_seo_event_deliveries;
mod m20260602_000006_create_seo_index_tracking;
mod m20260716_000007_add_redirect_cache_cursor_index;

use sea_orm_migration::prelude::*;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260419_000002_expand_seo_locale_columns::Migration),
        Box::new(m20260419_000003_create_seo_tables::Migration),
        Box::new(m20260420_000004_create_seo_bulk_tables::Migration),
        Box::new(m20260421_000005_create_seo_event_deliveries::Migration),
        Box::new(m20260602_000006_create_seo_index_tracking::Migration),
        Box::new(m20260716_000007_add_redirect_cache_cursor_index::Migration),
    ]
}
