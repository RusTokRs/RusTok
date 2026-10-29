//! # RusToK Blog Storefront
//!
//! Leptos storefront UI package for the `rustok-blog` module.
//! Provides published post viewing, RSS feed generation, and comment thread integrations.

#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
mod comments_pagination;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
mod core;
mod i18n;
mod model;
mod transport;
mod ui;

#[cfg(feature = "comment-island")]
pub use ui::BlogCommentComposer;
#[cfg(any(feature = "ssr", not(feature = "comment-island")))]
pub use ui::BlogView;

#[cfg(feature = "ssr")]
pub use transport::feed::render_rss_feed;

#[cfg(test)]
mod tests;
#[cfg(test)]
mod contract_tests;
