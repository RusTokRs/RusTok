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

    #[error(
        "Relation between product {product_id} and {related_product_id} with type '{relation_type}' already exists"
    )]
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

    #[error("Relation {0} occurs more than once in a reorder request")]
    DuplicateRelationInReorder(Uuid),

    #[error(
        "Reorder request must contain every relation in its group exactly once (expected {expected}, received {received})"
    )]
    IncompleteReorder { expected: usize, received: usize },

    #[error("Relation position {position} is outside the allowed range 0..={maximum}")]
    PositionOutOfRange { position: i32, maximum: usize },
}

pub type ProductRelationResult<T> = Result<T, ProductRelationError>;
