mod m20260720_000001_create_moderation_core;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260720_000001_create_moderation_core::Migration)]
}
