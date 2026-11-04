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

pub mod dto;
pub(crate) mod entities;
pub mod error;
pub mod migrations;
pub mod module;
pub mod ports;
pub mod services;

#[cfg(test)]
mod tests;

pub use dto::{
    CreateProductRelationInput, MAX_RELATION_TYPE_LEN, ProductRelationDto, RelationType,
    ReorderProductRelationsInput, UpdateProductRelationInput,
};
pub use error::{ProductRelationError, ProductRelationResult};
pub use module::ProductRelationsModule;
pub use ports::ProductRelationsPort;
pub use services::{
    CREATE_RELATION_OPERATION, DELETE_RELATION_OPERATION, PRODUCT_RELATION_OWNER_SLUG,
    ProductRelationCommandContext, ProductRelationCommandError, ProductRelationService,
    REORDER_RELATIONS_OPERATION, RELATION_CALLER_IDEMPOTENCY_NAMESPACE,
    UPDATE_RELATION_OPERATION, relation_command_error, scoped_caller_idempotency_key,
};
