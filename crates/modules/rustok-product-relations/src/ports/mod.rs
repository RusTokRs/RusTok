/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Port trait defining the cross-boundary application contract for Product Relations.

use async_trait::async_trait;
use uuid::Uuid;

use crate::dto::{
    CreateProductRelationInput, ProductRelationDto, RelationType, ReorderProductRelationsInput,
    UpdateProductRelationInput,
};
use crate::error::ProductRelationResult;

/// Primary service port for managing product relations.
#[async_trait]
pub trait ProductRelationsPort: Send + Sync {
    /// Lists all relations for a source product, optionally filtered by relation type.
    ///
    /// # Errors
    /// Returns [`ProductRelationError::Database`](crate::error::ProductRelationError::Database)
    /// if a query failure occurs.
    async fn list_relations(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        relation_type: Option<RelationType>,
    ) -> ProductRelationResult<Vec<ProductRelationDto>>;

    /// Lists all reverse relations where the given product is the target (`related_product_id`).
    ///
    /// # Errors
    /// Returns [`ProductRelationError::Database`](crate::error::ProductRelationError::Database)
    /// if a query failure occurs.
    async fn list_reverse_relations(
        &self,
        tenant_id: Uuid,
        related_product_id: Uuid,
        relation_type: Option<RelationType>,
    ) -> ProductRelationResult<Vec<ProductRelationDto>>;

    /// Retrieves a single product relation by its unique identifier.
    ///
    /// # Errors
    /// Returns [`ProductRelationError::RelationNotFound`](crate::error::ProductRelationError::RelationNotFound)
    /// if the relation does not exist in the given tenant scope.
    async fn get_relation(
        &self,
        tenant_id: Uuid,
        relation_id: Uuid,
    ) -> ProductRelationResult<ProductRelationDto>;

    /// Creates a new directed relation between two products.
    ///
    /// # Errors
    /// - [`ProductRelationError::SelfRelationNotAllowed`](crate::error::ProductRelationError::SelfRelationNotAllowed)
    ///   if source and target product IDs are identical.
    /// - [`ProductRelationError::ProductNotFound`](crate::error::ProductRelationError::ProductNotFound)
    ///   if either the source or target product does not exist in the tenant.
    /// - [`ProductRelationError::RelationAlreadyExists`](crate::error::ProductRelationError::RelationAlreadyExists)
    ///   if the `(tenant_id, product_id, related_product_id, relation_type)` tuple already exists.
    async fn create_relation(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto>;

    /// Updates a relation's position or JSON metadata.
    ///
    /// # Errors
    /// - [`ProductRelationError::RelationNotFound`](crate::error::ProductRelationError::RelationNotFound)
    ///   if the relation does not exist in the tenant.
    /// - [`ProductRelationError::PositionOutOfRange`](crate::error::ProductRelationError::PositionOutOfRange)
    ///   if the new position exceeds allowed contiguous bounds.
    async fn update_relation(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        relation_id: Uuid,
        input: UpdateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto>;

    /// Deletes a relation and shifts higher positions down to preserve contiguous ordering.
    ///
    /// # Errors
    /// - [`ProductRelationError::RelationNotFound`](crate::error::ProductRelationError::RelationNotFound)
    ///   if the relation does not exist in the tenant.
    async fn delete_relation(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        relation_id: Uuid,
    ) -> ProductRelationResult<()>;

    /// Atomically reorders relations of a given type for a product.
    ///
    /// # Errors
    /// - [`ProductRelationError::DuplicateRelationInReorder`](crate::error::ProductRelationError::DuplicateRelationInReorder)
    ///   if the ordered relation ID list contains duplicate entries.
    /// - [`ProductRelationError::IncompleteReorder`](crate::error::ProductRelationError::IncompleteReorder)
    ///   if the list does not match the full set of relations in the group exactly.
    async fn reorder_relations(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: ReorderProductRelationsInput,
    ) -> ProductRelationResult<Vec<ProductRelationDto>>;
}
