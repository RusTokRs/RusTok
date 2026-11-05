pub mod comment;
pub mod comments_list;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod leptos;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod share;

#[cfg(feature = "comment-island")]
pub use comment::BlogDiscussion;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use leptos::{BlogDraftPreview, BlogView};
