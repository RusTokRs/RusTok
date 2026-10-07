#[cfg(feature = "comment-island")]
mod comment;
pub mod comments_list;
pub mod share;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod leptos;

#[cfg(feature = "comment-island")]
pub use comment::BlogCommentComposer;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use leptos::BlogView;
