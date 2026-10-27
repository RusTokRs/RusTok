//! # RusToK Tenant Module
//!
//! Multi-tenancy lifecycle and tenant-scoped module-state module for RusToK.
//!
//! Provides tenant CRUD, domain lookup, locale-policy aggregate management,
//! outbox event publishing, and tenant-scoped RBAC permissions.

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;

mod settings_schema;

pub use dto::{CreateTenantInput, TenantModuleResponse, TenantResponse, UpdateTenantInput};
pub use error::TenantError;
pub use module::TenantModule;
pub use ports::{
    in_process_tenant_storefront_ports, PortActor, PortCallPolicy, PortContext, PortError,
    PortErrorKind, ReplaceTenantLocalePolicyRequest, TenantLocale, TenantLocalePolicyEntry,
    TenantLocalePolicyPort, TenantLocalePolicyProjection, TenantReadPort, TenantReadProjection,
    TenantReadRequest, TenantReadSelector,
};
pub use services::TenantService;

#[cfg(test)]
mod contract_tests;

