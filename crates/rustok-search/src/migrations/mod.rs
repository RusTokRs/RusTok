mod m20260324_000001_create_search_settings;
mod m20260324_000002_create_search_documents;
mod m20260325_000003_create_search_query_logs;
mod m20260325_000004_create_search_query_clicks;
mod m20260325_000005_create_search_dictionaries;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260324_000001_create_search_settings::Migration),
        Box::new(m20260324_000002_create_search_documents::Migration),
        Box::new(m20260325_000003_create_search_query_logs::Migration),
        Box::new(m20260325_000004_create_search_query_clicks::Migration),
        Box::new(m20260325_000005_create_search_dictionaries::Migration),
    ]
}
