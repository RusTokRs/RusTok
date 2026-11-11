//! Blog cross-boundary ports and orchestration bridge types.
//!
//! Exposes owner in-transaction commands and types for content orchestration
//! (`rustok-content-orchestration`) and cross-module boundaries without leaking
//! private persistence entities.

pub use crate::services::orchestration::*;

pub mod taxonomy;
pub use taxonomy::{
    AsBlogTaxonomyConnection, BlogCategoryDeleteCleanupPort, BlogTaxonomyCategoryLocaleCopy,
    BlogTaxonomyCategorySnapshot, BlogTaxonomyConnection, BlogTaxonomyCreateTagInput,
    BlogTaxonomyCreatedTag, BlogTaxonomyPlacement, BlogTaxonomyPort, BlogTaxonomySyncCategoryInput,
    BlogTaxonomyTagSnapshot, BlogTaxonomyUpdateTagInput, NoopBlogTaxonomyPort,
};
