//! # RusToK Blog Storefront
//!
//! Leptos storefront UI package for the `rustok-blog` module.
//! Provides published post viewing, RSS feed generation, and comment thread integrations.

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
mod core;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use core::{build_draft_preview_copy, comments_pagination, parse_preview_token_from_fragment};
mod i18n;
mod model;
mod transport;
mod ui;

#[cfg(feature = "comment-island")]
pub use ui::BlogCommentComposer;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use ui::BlogDraftPreview;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use ui::BlogView;

#[cfg(feature = "ssr")]
pub use transport::feed::render_rss_feed;

#[cfg(test)]
mod tests;
