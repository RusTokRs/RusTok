use sea_orm::DbErr;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ProductRelationError {
    #[error("Database error: {0}")]
    Database(#[from] DbErr),

    #[error("Outbox error: {0}")]
    Outbox(String),

    #[error("Self-relation is not allowed: product {0} cannot be related to itself")]
    SelfRelationNotAllowed(Uuid),

    #[error("Relation between product {product_id} and {related_product_id} with type '{relation_type}' already exists")]
    RelationAlreadyExists {
        product_id: Uuid,
        related_product_id: Uuid,
        relation_type: String,
    },

    #[error("Relation {0} not found")]
    RelationNotFound(Uuid),

    #[error("Product {0} not found")]
    ProductNotFound(Uuid),

    #[error("Invalid relation type: '{0}'")]
    InvalidRelationType(String),

    #[error("Invalid relation input: {0}")]
    InvalidInput(String),
}

pub type ProductRelationResult<T> = Result<T, ProductRelationError>;
