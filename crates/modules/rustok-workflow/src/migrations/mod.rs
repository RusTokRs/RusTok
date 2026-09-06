mod m20260316_000006_create_workflows;
mod m20260316_000007_alter_workflows_add_failure_tracking;
mod m20260627_000008_add_event_execution_idempotency;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260316_000006_create_workflows::Migration),
        Box::new(m20260316_000007_alter_workflows_add_failure_tracking::Migration),
        Box::new(m20260627_000008_add_event_execution_idempotency::Migration),
    ]
}
