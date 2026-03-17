mod m20260316_000006_create_workflows;
mod m20260316_000007_alter_workflows_add_failure_tracking;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260316_000006_create_workflows::Migration),
        Box::new(m20260316_000007_alter_workflows_add_failure_tracking::Migration),
    ]
}
