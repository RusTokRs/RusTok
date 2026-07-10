mod m20260710_000001_rig_provider_profiles;

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
    vec![Box::new(m20260710_000001_rig_provider_profiles::Migration)]
}
