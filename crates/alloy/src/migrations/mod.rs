mod m20260302_000001_create_scripts;
mod m20260302_000002_create_script_executions;
mod m20260718_000003_create_script_revisions;
mod m20260718_000004_create_script_reviews;
mod m20260718_000005_create_script_test_runs;
mod m20260726_000006_add_execution_evidence;
mod m20260726_000007_add_imported_release_lineage;
mod m20260726_000008_create_release_imports;
mod m20260825_000009_create_component_candidates;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260302_000001_create_scripts::Migration),
        Box::new(m20260302_000002_create_script_executions::Migration),
        Box::new(m20260718_000003_create_script_revisions::Migration),
        Box::new(m20260718_000004_create_script_reviews::Migration),
        Box::new(m20260718_000005_create_script_test_runs::Migration),
        Box::new(m20260726_000006_add_execution_evidence::Migration),
        Box::new(m20260726_000007_add_imported_release_lineage::Migration),
        Box::new(m20260726_000008_create_release_imports::Migration),
        Box::new(m20260825_000009_create_component_candidates::Migration),
    ]
}
