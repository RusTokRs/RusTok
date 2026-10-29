//! # rustok-media-admin
//!
//! Leptos admin UI package for the `rustok-media` module.
//!
//! Exposes media library browsing, multi-file uploads, image optimization,
//! and asset usage tracking for platform administrators.

mod core;
mod i18n;
mod model;
mod transport;
mod ui;

pub use model::{MediaListItem, MediaListPayload};
pub use transport::{ApiError as MediaTransportError, fetch_media_library, upload_media};
pub use ui::leptos::MediaAdmin;

#[cfg(test)]
mod contract_tests;
