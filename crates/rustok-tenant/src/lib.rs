pub mod dto;
pub mod entities;
pub mod error;
pub mod services;

pub use dto::{
    CreateTenantInput, TenantModuleResponse, TenantResponse, ToggleModuleInput, UpdateTenantInput,
};
pub use error::TenantError;
pub use services::TenantService;

use async_trait::async_trait;
use rustok_core::module::{HealthStatus, MigrationSource, ModuleKind, RusToKModule};
use sea_orm_migration::MigrationTrait;

pub struct TenantModule;

impl MigrationSource for TenantModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for TenantModule {
    fn slug(&self) -> &'static str {
        "tenant"
    }

    fn name(&self) -> &'static str {
        "Tenant"
    }

    fn description(&self) -> &'static str {
        "Multi-tenancy helpers and tenant metadata."
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn kind(&self) -> ModuleKind {
        ModuleKind::Core
    }

    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }
}

#[cfg(test)]
mod contract_tests;
