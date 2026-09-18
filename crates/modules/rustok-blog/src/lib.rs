//! Blog domain module for the RusToK platform.
//!
//! The crate owns Blog posts, Blog-specific Category membership/settings and tag
//! attachment state, while integrating with canonical platform owners through
//! explicit contracts. Physical source placement follows the canonical native
//! module layout; public compatibility is preserved through deliberate re-exports.

mod domain;
mod integrations;
mod module;

pub mod controllers;
pub mod dto;
pub mod entities;
pub mod error;
pub mod graphql;
pub mod migrations;
pub mod services;

pub use controllers::openapi;
pub use domain::{richtext, state_machine};
pub use dto::{
    CategoryListItem, CategoryListResponse, CategoryResponse, CommentListItem, CommentResponse,
    CreateCategoryInput, CreateCommentInput, CreatePostInput, CreateTagInput, ListCategoriesFilter,
    ListCommentsFilter, ListTagsFilter, ModerateCommentInput, ModerateCommentStatus, PostListQuery,
    PostListResponse, PostResponse, PostSummary, TagListItem, TagResponse, UpdateCategoryInput,
    UpdateCommentInput, UpdatePostInput, UpdateTagInput,
};
pub use entities::*;
pub use error::{BlogError, BlogResult};
pub use graphql::{BlogMutation, BlogQuery};
pub use integrations::public_comments_snapshot;
pub use integrations::public_comments_snapshot::{
    PublicCommentsAvailability, PublicCommentsRead, PublicCommentsSnapshotStore,
    list_public_comments_with_snapshot,
};
pub use integrations::reaction_subject::{
    BLOG_POST_REACTION_KIND, BLOG_REACTION_SOURCE, BLOG_REACTION_V1_KEY,
    BlogReactionSubjectProviderFactory,
};
pub use module::BlogModule;
pub use rustok_comments::CommentsThreadPort;
pub use services::{CategoryService, CommentService, PostService, TagService};
pub use state_machine::{
    Archived, BlogPost, BlogPostStatus, CommentStatus, Draft, Published, ToBlogPostStatus,
};

#[cfg(test)]
mod tests;
