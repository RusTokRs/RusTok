mod m20260302_000001_create_scripts;
mod m20260302_000002_create_script_executions;
mod m20260718_000003_create_script_revisions;
mod m20260718_000004_create_script_reviews;
mod m20260718_000005_create_script_test_runs;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260302_000001_create_scripts::Migration),
        Box::new(m20260302_000002_create_script_executions::Migration),
        Box::new(m20260718_000003_create_script_revisions::Migration),
        Box::new(m20260718_000004_create_script_reviews::Migration),
        Box::new(m20260718_000005_create_script_test_runs::Migration),
    ]
}
