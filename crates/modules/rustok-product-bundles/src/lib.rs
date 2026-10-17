use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationDependencyDescriptor, MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;
use uuid::Uuid;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod services;

pub use dto::*;
pub use error::{BundleError, BundleResult};
pub use ports::BundlePort;
pub use services::{
    ADD_BUNDLE_ITEM_OPERATION, BundleCommandContext, BundleCommandError, BundleService,
    CREATE_BUNDLE_OPERATION, DELETE_BUNDLE_OPERATION, PRODUCT_BUNDLE_OWNER_SLUG,
    REMOVE_BUNDLE_ITEM_OPERATION, UPDATE_BUNDLE_OPERATION, bundle_command_error,
};

/// Stable namespace for caller keys accepted by every Product Bundle ingress.
///
/// Kept compatible with the initial Commerce GraphQL namespace so existing retries retain their
/// receipt identity as native and GraphQL adapters converge on this owner helper.
pub const BUNDLE_CALLER_IDEMPOTENCY_NAMESPACE: &str = "commerce-graphql-bundle";

/// Derives the owner receipt identity for a caller-supplied Bundle command key.
///
/// Callers must reject empty and overlong caller keys before deriving this bounded hashed key.
pub fn scoped_caller_idempotency_key(
    tenant_id: Uuid,
    user_id: Uuid,
    operation: &str,
    subject_id: Option<Uuid>,
    caller_key: &str,
) -> String {
    rustok_outbox::idempotency::scoped_caller_idempotency_key(
        BUNDLE_CALLER_IDEMPOTENCY_NAMESPACE,
        tenant_id,
        user_id,
        operation,
        subject_id,
        caller_key,
    )
}

pub struct ProductBundlesModule;

#[async_trait]
impl RusToKModule for ProductBundlesModule {
    fn slug(&self) -> &'static str {
        "product_bundles"
    }

    fn name(&self) -> &'static str {
        "Product Bundles"
    }

    fn description(&self) -> &'static str {
        "Product bundles, kits, configurable sets, and package discounts"
    }

    fn version(&self) -> &'static str {
        env!("CARGO_PKG_VERSION")
    }

    fn dependencies(&self) -> &[&'static str] {
        &["product"]
    }

    fn permissions(&self) -> Vec<Permission> {
        vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE]
    }
}

impl MigrationSource for ProductBundlesModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
