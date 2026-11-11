//! # rustok-pages-storefront
//!
//! Leptos storefront UI package for the `rustok-pages` CMS static pages module.
//!
//! Provides the canonical `PagesView` root component for published page delivery,
//! headless/monolith dual-path data access, direct SSR rendering of published landing artifacts,
//! and interactive action bridges without `<iframe>` wrappers.
//!
//! The storefront renders only the published landing of a page. The authoring document is
//! never requested or rendered here; authenticated inline editing is the separate
//! `inline-edit` surface.

mod builder;
#[cfg(test)]
mod contract_tests;
mod core;
mod i18n;
#[cfg(feature = "inline-edit")]
mod inline_edit;
mod model;
mod transport;
mod ui;

pub use builder::PublishedLandingBody;
#[cfg(feature = "inline-edit")]
pub use inline_edit::{
    PagesAuthenticatedInlineEditSurface, PagesInlineEditBootstrap, commit_pages_inline_edit,
    fetch_pages_inline_edit_bootstrap,
};
pub use model::PublishedLanding;
pub use transport::{
    StorefrontPageRouteDecision, StorefrontPageRouteDisposition, resolve_storefront_page_route,
};
pub use ui::leptos::PagesView;
