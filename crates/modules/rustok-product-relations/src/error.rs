/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Domain and persistence error types for Product Relations.

use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

/// Typed errors produced during product relation validation, persistence, or ordering.
#[derive(Debug, Error)]
pub enum ProductRelationError {
    /// Underlying relational database error.
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    /// Receipt logging or outbox ledger failure.
    #[error("Outbox error: {0}")]
    Outbox(String),

    /// Attempt to relate a product to itself.
    #[error("Self-relation is not allowed: product {0} cannot be related to itself")]
    SelfRelationNotAllowed(Uuid),

    /// Duplicate association between the same source, target, and relation type.
    #[error(
        "Relation between product {product_id} and {related_product_id} with type '{relation_type}' already exists"
    )]
    RelationAlreadyExists {
        /// Source product ID.
        product_id: Uuid,
        /// Related target product ID.
        related_product_id: Uuid,
        /// Canonical relation type key.
        relation_type: String,
    },

    /// Specified relation ID does not exist in the tenant scope.
    #[error("Relation {0} not found")]
    RelationNotFound(Uuid),

    /// Referenced source or target product was not found in the tenant scope.
    #[error("Product {0} not found")]
    ProductNotFound(Uuid),

    /// Relation type contains illegal characters, is empty, or exceeds length bounds.
    #[error("Invalid relation type: '{0}'")]
    InvalidRelationType(String),

    /// Malformed input parameters (e.g. invalid JSON metadata).
    #[error("Invalid relation input: {0}")]
    InvalidInput(String),

    /// Reordering input lists the same relation ID multiple times.
    #[error("Relation {0} occurs more than once in a reorder request")]
    DuplicateRelationInReorder(Uuid),

    /// Reordering input does not match the exact set of relations in the group.
    #[error(
        "Reorder request must contain every relation in its group exactly once (expected {expected}, received {received})"
    )]
    IncompleteReorder {
        /// Number of relations in the stored group.
        expected: usize,
        /// Number of relations provided in the reorder request.
        received: usize,
    },

    /// Relation position is negative or exceeds the upper contiguous boundary.
    #[error("Relation position {position} is outside the allowed range 0..={maximum}")]
    PositionOutOfRange {
        /// Requested or calculated position.
        position: i32,
        /// Maximum allowed contiguous position.
        maximum: usize,
    },
}

/// Convenience result alias for Product Relation operations.
pub type ProductRelationResult<T> = Result<T, ProductRelationError>;
