use async_trait::async_trait;
use rustok_core::permissions::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod services;

pub use dto::*;
pub use error::{RegionError, RegionResult};
pub use services::RegionService;

pub struct RegionModule;

#[async_trait]
impl RusToKModule for RegionModule {
    fn slug(&self) -> &'static str {
        "region"
    }

    fn name(&self) -> &'static str {
        "Region"
    }

    fn description(&self) -> &'static str {
        "Default region submodule in the ecommerce family"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![
            Permission::REGIONS_CREATE,
            Permission::REGIONS_READ,
            Permission::REGIONS_UPDATE,
            Permission::REGIONS_DELETE,
            Permission::REGIONS_LIST,
            Permission::REGIONS_MANAGE,
        ]
    }
}

impl MigrationSource for RegionModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }
}
