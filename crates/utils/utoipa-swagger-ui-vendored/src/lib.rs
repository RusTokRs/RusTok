//! # utoipa-swagger-ui-vendored
//!
//! Vendored Swagger UI asset bundle for the RusToK API docs surfaces. The
//! crate exposes the pinned asset set as a single constant so no build step
//! or runtime download is required to serve the bundled UI.

mod generated;

#[cfg(test)]
mod contract_tests;

pub use generated::SWAGGER_UI_VENDORED;
