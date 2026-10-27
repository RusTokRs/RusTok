/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! # RusToK Product Relations Module
//!
//! Merchandising product associations for cross-sells, up-sells, related products,
//! accessories, alternatives, and custom relationship types.
//!
//! ## Responsibilities & Guarantees
//!
//! - **Directed associations**: Directed links between source and target catalog products in the
//!   `product_relations` table.
//! - **Tenant isolation**: Enforces tenant scoping and tenant-coupled foreign keys to source and
//!   target products; self-relations are rejected.
//! - **Contiguous ordering**: Maintains a complete, non-negative contiguous ordering `0..N` per
//!   `(tenant_id, product_id, relation_type)`. Reordering requests must supply the complete group.
//! - **Durable receipts**: Create, update, delete, and reorder commands complete durable owner
//!   receipts inside the same transaction as the write, replaying stored responses on retries.
//! - **Deadlock-free locking**: Concurrent writes take pessimistic product locks ordered by UUID
//!   and reload models to eliminate check-then-act race conditions.
//!
//! ## Entry Points
//!
//! - [`ProductRelationsPort`]: Primary port trait for reading, creating, updating, deleting,
//!   and reordering product relations.
//! - [`ProductRelationService`]: Domain application service orchestrating transactions,
//!   validation, pessimistic locking, and receipt logging.
//! - [`ProductRelationsModule`]: Platform module lifecycle and migration provider descriptor.

use sha2::{Digest, Sha256};
use uuid::Uuid;

pub mod dto;
pub mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;

#[cfg(test)]
mod contract_tests;

pub use dto::{
    CreateProductRelationInput, ProductRelationDto, RelationType, ReorderProductRelationsInput,
    UpdateProductRelationInput, MAX_RELATION_TYPE_LEN,
};
pub use error::{ProductRelationError, ProductRelationResult};
pub use module::ProductRelationsModule;
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
