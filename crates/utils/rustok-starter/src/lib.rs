//! # rustok-starter
//!
//! Canonical declarative Starter Blueprint models and the data import engine.
//! The crate renders a blueprint into the platform's starter commands without
//! owning any module's write semantics.

pub mod drivers;
pub mod embedded;
pub mod error;
pub mod importer;
pub mod model;

#[cfg(test)]
mod contract_tests;

pub use embedded::default_starter;
pub use error::{StarterError, StarterResult};
pub use importer::StarterEngine;
pub use model::{
    BlogPostStarter, BlogPostTranslationStarter, BlogStarter, ForumCategoryStarter,
    ForumReplyStarter, ForumStarter, ForumTopicStarter, NavigationMenuItemStarter,
    NavigationMenuStarter, NavigationStarter, PageBodyStarter, PageStarter, StarterBlueprint,
    StarterContent, StarterExecutionReport, TaxonomyCategoryStarter, TaxonomyStarter,
};
