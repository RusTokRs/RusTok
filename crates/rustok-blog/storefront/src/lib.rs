#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
mod comments_pagination;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
mod core;
mod i18n;
mod model;
mod transport;
mod ui;

#[cfg(target_arch = "wasm32")]
pub use ui::BlogCommentComposer;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use ui::BlogView;
