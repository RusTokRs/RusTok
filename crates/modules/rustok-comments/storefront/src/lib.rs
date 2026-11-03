//! # RusToK Comments Storefront Support
//!
//! Reusable Leptos storefront authoring UI package for the `rustok-comments` capability.
//! Provides comment composer state, rich-text frame integration, and validation logic.

mod core;
mod ui;

pub use core::{is_richtext_blank, validate_comment_submission, CommentComposerValidationError};
pub use ui::{CommentComposer, CommentComposerCopy};

#[cfg(test)]
mod contract_tests;
