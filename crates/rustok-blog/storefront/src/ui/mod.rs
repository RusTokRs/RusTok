#[cfg(feature = "comment-island")]
mod comment;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub mod leptos;

#[cfg(all(feature = "comment-island", target_arch = "wasm32"))]
pub use comment::BlogCommentComposer;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use leptos::BlogView;
