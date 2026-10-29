//! # RusToK Blog Admin
//!
//! Leptos admin UI package for the `rustok-blog` module.
//! Provides post authoring, category management, comment moderation panels, and Fly page-builder integrations.

#![recursion_limit = "512"]

mod core;
mod i18n;
mod model;
mod moderation;
mod page_builder;
mod transport;
mod ui;


pub use page_builder::{
    BlogContributionAdapter, BlogWidgetOwnerSchemaRef, BlogWidgetPropertyEditorModel,
    BlogWidgetRenderModel, blog_contribution_manifest, blog_fly_registry_set,
    blog_full_admin_contribution_policy, blog_widget_contribution,
    blog_widget_preview_contribution, build_blog_admin_contribution_registry,
    register_blog_fly_widgets,
};
pub use ui::BlogAdmin;


#[cfg(test)]
mod tests;
