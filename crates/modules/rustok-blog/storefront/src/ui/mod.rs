#[cfg(feature = "comment-island")]
mod comment;
pub mod comments_list;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod leptos;
pub mod share;

#[cfg(feature = "comment-island")]
pub use comment::BlogCommentComposer;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use leptos::BlogView;
