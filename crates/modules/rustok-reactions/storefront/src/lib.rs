//! # rustok-reactions-storefront
//!
//! Leptos storefront UI package for the `rustok-reactions` module.
//!
//! Provides producer-neutral reaction bars, like/reaction action buttons,
//! and reaction count presentations.

pub mod i18n;
pub mod model;
pub mod transport;
pub mod ui;

pub use model::{ReactionAction, ReactionSnapshotView, ReactionSubjectUiRef};
pub use ui::ReactionBar;

#[cfg(test)]
mod contract_tests;
