mod m20260710_000001_rig_provider_profiles;
mod m20260712_000001_provider_targets;
mod m20260712_000002_approval_batches;

use rustok_core::MigrationSource;
use sea_orm_migration::MigrationTrait;

/// Owner-owned migration source for the AI capability boundary.
#[derive(Debug, Default, Clone, Copy)]
pub struct AiMigrationSource;

impl MigrationSource for AiMigrationSource {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations()
    }
}

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260710_000001_rig_provider_profiles::Migration),
        Box::new(m20260712_000001_provider_targets::Migration),
        Box::new(m20260712_000002_approval_batches::Migration),
    ]
}
