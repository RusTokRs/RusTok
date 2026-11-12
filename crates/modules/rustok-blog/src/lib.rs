//! # RusToK Blog
//!
//! Blog domain module for the RusToK platform.
//!
//! The crate owns Blog posts, Blog-specific Category membership/settings and tag
//! attachment state, while integrating with canonical platform owners through
//! explicit contracts. Physical source placement follows the canonical native
//! module layout; public compatibility is preserved through deliberate re-exports.

pub(crate) mod domain;
mod integrations;
pub mod module;

pub mod controllers;
pub mod dto;
mod entities;
pub mod error;
pub mod graphql;
pub mod migrations;
pub mod normalization;
pub mod ports;
pub mod services;

pub use services::search_projection;
pub use services::search_projection::{
    BlogSearchProjectionSource, BlogSearchProjectionSourceFactory,
};

pub use controllers::openapi;
pub use domain::{richtext, state_machine};
pub use dto::{
    ArchivePostInput, BulkOperationFailure, BulkOperationResult, CategoryListItem,
    CategoryListResponse, CategoryResponse, CommentListItem, CommentResponse, CreateCategoryInput,
    CreateCommentInput, CreatePostInput, CreateTagInput, ListCategoriesFilter, ListCommentsFilter,
    ListTagsFilter, ModerateCommentInput, ModerateCommentStatus, PostListQuery, PostListResponse,
    PostResponse, PostSortField, PostSortOrder, PostSummary, PublicPostPage, PublicPostsPageQuery,
    PublishedPostCursor, TagListItem, TagResponse, UpdateCategoryInput, UpdateCommentInput,
    UpdatePostInput, UpdateTagInput,
};
pub use dto::{
    BlogWidgetCatalog, BlogWidgetInput, BlogWidgetIssue, BlogWidgetPayload, BlogWidgetPreview,
    BlogWidgetSchema, BlogWidgetValidation,
};
pub use error::{BlogError, BlogPublicError, BlogResult};
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
pub use ports::BlogCategoryDeleteCleanupPort as TaxonomyCategoryDeleteCleanupPort;
pub use ports::{
    BlogCategoryDeleteCleanupPort, BlogTaxonomyConnection, BlogTaxonomyPort,
    NoopBlogCategoryDeleteCleanupPort, NoopBlogTaxonomyPort,
};
pub use ports::{
    CreatePromotedPostInput, DemotedPostRecord, DemotedPostTranslationRecord,
    PromotedPostTranslationInput, create_promoted_post_in_tx, delete_demoted_post_in_tx,
    ensure_blog_category_exists_in_tx, ensure_blog_slug_unique_in_tx, load_post_for_demotion_in_tx,
    load_post_tag_ids_in_tx,
};
pub use rustok_comments::CommentsThreadPort;
pub use services::BlogWidgetService;
pub use services::{
    BLOG_ROUTE_PREFIX, BlogPostRedirect, BlogPostRouteOwner, CANONICAL_POST_ROUTE_LOCALE,
    CategoryService, CommentService, PostService, PreviewToken, PreviewTokenService, TagService,
    canonical_post_route, remove_post_tag_links_in_tx, replace_post_tag_links_in_tx,
};
pub use state_machine::BlogPostStatus;

#[cfg(test)]
mod tests;
