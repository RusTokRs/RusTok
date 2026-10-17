use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;
use uuid::Uuid;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod ports;
pub mod services;

pub use dto::*;
pub use error::{ProductRelationError, ProductRelationResult};
pub use ports::ProductRelationsPort;
pub use services::{
    CREATE_RELATION_OPERATION, DELETE_RELATION_OPERATION, PRODUCT_RELATION_OWNER_SLUG,
    ProductRelationCommandContext, ProductRelationCommandError, ProductRelationService,
    REORDER_RELATIONS_OPERATION, UPDATE_RELATION_OPERATION, relation_command_error,
};

/// Stable namespace for caller keys accepted by every Product Relation ingress.
///
/// Kept compatible with the initial Commerce GraphQL namespace so existing retries retain their
/// receipt identity as native and GraphQL adapters converge on this owner helper.
pub const RELATION_CALLER_IDEMPOTENCY_NAMESPACE: &str = "commerce-graphql-relation";

/// Derives the owner receipt identity for a caller-supplied Relation command key.
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
        RELATION_CALLER_IDEMPOTENCY_NAMESPACE,
        tenant_id,
        user_id,
        operation,
        subject_id,
        caller_key,
    )
}

pub struct ProductRelationsModule;

#[async_trait]
impl RusToKModule for ProductRelationsModule {
    fn slug(&self) -> &'static str {
        "product_relations"
    }

    fn name(&self) -> &'static str {
        "Product Relations"
    }

    fn description(&self) -> &'static str {
        "Product relations, merchandising associations (cross-sells, up-sells, related, accessories, alternatives) and reordering"
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

impl MigrationSource for ProductRelationsModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        migrations::migrations()
    }

    fn migration_dependencies(&self) -> Vec<rustok_core::MigrationDependencyDescriptor> {
        migrations::migration_dependencies()
    }
}
