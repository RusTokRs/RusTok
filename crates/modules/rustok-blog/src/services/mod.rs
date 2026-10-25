//! Services for the Blog module

mod category;
mod category_command;
mod category_delete;
pub(crate) mod category_name_projection;
mod category_owner;
pub(crate) mod category_taxonomy_sync;
mod comment;
mod comment_projection;
pub(crate) mod post;
pub(crate) mod preview_token;
mod rbac;
pub mod search_projection;
#[cfg(feature = "runtime")]
pub(crate) mod scheduler;
pub(crate) mod tag;
mod taxonomy_translation_owner;

pub use category_command::CategoryCommandService;
pub use category_owner::CategoryService;
pub use comment::CommentService;
pub use comment_projection::BlogCommentProjectionHandler;
pub(crate) use post::BLOG_POST_TARGET_TYPE;
pub use post::{
    BLOG_ROUTE_PREFIX, BlogPostRedirect, BlogPostRouteOwner, CANONICAL_POST_ROUTE_LOCALE,
    PostService, canonical_post_route,
};
#[cfg(any(feature = "runtime", test))]
pub(crate) use post::{MAX_SCHEDULED_PUBLISH_BATCH, status_to_storage};
pub(crate) use post::MAX_POST_ARTICLE_BYTES;
pub(crate) use post::{is_post_visible_for_channel, load_post_subject_snapshot};
pub(crate) use rbac::can_read_non_public_posts;
pub use preview_token::{PreviewToken, PreviewTokenService};
pub use tag::{
    TagService, remove_post_tag_links_in_tx, replace_post_tag_links_in_tx,
};
pub use taxonomy_translation_owner::BlogTaxonomyTranslationOwner;

#[cfg(test)]
mod tag_tenant_integrity_tests;
