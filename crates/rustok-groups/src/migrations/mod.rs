mod m20260721_000001_create_groups;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![Box::new(m20260721_000001_create_groups::Migration)]
}
