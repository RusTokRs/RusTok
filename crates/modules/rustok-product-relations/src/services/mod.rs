pub mod receipts;
pub mod relation_service;

pub use receipts::{
    CREATE_RELATION_OPERATION, PRODUCT_RELATION_OWNER_SLUG, ProductRelationCommandContext,
    ProductRelationCommandError, relation_command_error,
};
pub use relation_service::ProductRelationService;
