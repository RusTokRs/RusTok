mod m20260302_000001_create_scripts;
mod m20260302_000002_create_script_executions;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260302_000001_create_scripts::Migration),
        Box::new(m20260302_000002_create_script_executions::Migration),
    ]
}
