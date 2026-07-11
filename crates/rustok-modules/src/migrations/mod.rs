mod m20260711_000001_module_artifact_installations;

use sea_orm_migration::prelude::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(
        m20260711_000001_module_artifact_installations::Migration,
    )]
}
