use async_trait::async_trait;
use uuid::Uuid;

use crate::dto::{
    CreateProductRelationInput, ProductRelationDto, RelationType, ReorderProductRelationsInput,
    UpdateProductRelationInput,
};
use crate::error::ProductRelationResult;

#[async_trait]
pub trait ProductRelationsPort: Send + Sync {
    /// List all relations for a product, optionally filtered by relation type.
    async fn list_relations(
        &self,
        tenant_id: Uuid,
        product_id: Uuid,
        relation_type: Option<RelationType>,
    ) -> ProductRelationResult<Vec<ProductRelationDto>>;

    /// List all reverse relations where the given product is the target.
    async fn list_reverse_relations(
        &self,
        tenant_id: Uuid,
        related_product_id: Uuid,
        relation_type: Option<RelationType>,
    ) -> ProductRelationResult<Vec<ProductRelationDto>>;

    /// Get a relation by its ID.
    async fn get_relation(
        &self,
        tenant_id: Uuid,
        relation_id: Uuid,
    ) -> ProductRelationResult<ProductRelationDto>;

    /// Create a new relation between two products.
    async fn create_relation(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto>;

    /// Update a relation's position or metadata.
    async fn update_relation(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        relation_id: Uuid,
        input: UpdateProductRelationInput,
    ) -> ProductRelationResult<ProductRelationDto>;

    /// Delete a relation.
    async fn delete_relation(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        relation_id: Uuid,
    ) -> ProductRelationResult<()>;

    /// Reorder relations of a given type for a product.
    async fn reorder_relations(
        &self,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: ReorderProductRelationsInput,
    ) -> ProductRelationResult<Vec<ProductRelationDto>>;
}
