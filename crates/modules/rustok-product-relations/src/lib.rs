use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{MigrationSource, RusToKModule};
use sea_orm_migration::MigrationTrait;
use sha2::{Digest, Sha256};
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
/// This deployed Product Relation namespace deliberately retains its original digest framing:
/// receipt keys are durable, so switching the formula would orphan in-flight and replayable
/// Commerce GraphQL receipts after an upgrade. A transition to a newly framed key must support
/// both receipt identities atomically rather than silently replacing this formula. Callers must
/// validate the caller key (non-empty, bounded length) before calling this.
pub fn scoped_caller_idempotency_key(
    tenant_id: Uuid,
    user_id: Uuid,
    operation: &str,
    subject_id: Option<Uuid>,
    caller_key: &str,
) -> String {
    let mut digest = Sha256::new();
    digest.update(tenant_id.as_bytes());
    digest.update(user_id.as_bytes());
    digest.update(operation.as_bytes());
    if let Some(subject_id) = subject_id {
        digest.update(subject_id.as_bytes());
    }
    digest.update(caller_key.trim().as_bytes());
    format!(
        "{RELATION_CALLER_IDEMPOTENCY_NAMESPACE}:{operation}:{}",
        hex::encode(digest.finalize())
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

#[cfg(test)]
mod tests {
    use super::scoped_caller_idempotency_key;
    use uuid::Uuid;

    #[test]
    fn deployed_relation_receipt_key_formula_stays_stable() {
        let key = scoped_caller_idempotency_key(
            Uuid::from_u128(0x11111111111111111111111111111111),
            Uuid::from_u128(0x22222222222222222222222222222222),
            "add_product_relation",
            None,
            " retry-123 ",
        );

        assert_eq!(
            key,
            "commerce-graphql-relation:add_product_relation:8346a5949e6c96a2844f67fbc37f4d112a3dd36bec196ef4ca8f9c61422ef7af"
        );
    }
}
