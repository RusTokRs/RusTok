mod shared;

mod m20260325_000001_create_channels;
mod m20260325_000002_create_channel_targets;
mod m20260325_000003_create_channel_module_bindings;
mod m20260325_000004_create_channel_oauth_apps;

use sea_orm_migration::MigrationTrait;

pub fn migrations() -> Vec<Box<dyn MigrationTrait>> {
    vec![
        Box::new(m20260325_000001_create_channels::Migration),
        Box::new(m20260325_000002_create_channel_targets::Migration),
        Box::new(m20260325_000003_create_channel_module_bindings::Migration),
        Box::new(m20260325_000004_create_channel_oauth_apps::Migration),
    ]
}
