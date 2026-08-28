//! Services for the Blog module

// CAT-8 retires the duplicate Translation provider while the compatibility
// Category mirror writer stays alive until the next bounded retirement slice.
// Some legacy read/lifecycle/provider-only seams therefore remain temporarily
// unreachable and are removed together with that mirror writer in CAT-9.
#[allow(dead_code)]
mod category;
mod category_command;
mod category_delete;
mod category_name_projection;
#[allow(dead_code)]
mod category_owner;
pub(crate) mod category_taxonomy_sync;
mod comment;
mod comment_projection;
mod post;
mod rbac;
mod tag;

pub use category_command::CategoryCommandService;
pub use category_owner::CategoryService;
pub use comment::CommentService;
pub use comment_projection::BlogCommentProjectionHandler;
pub use post::PostService;
pub(crate) use post::is_post_visible_for_channel;
pub use tag::TagService;
