//! # RusToK SEO Storefront Crate
//!
//! Provides storefront rendering adapters, framework-neutral SEO data structures,
//! dual-path transport query contracts, and HTML `<head>` meta-tag rendering
//! for client-facing storefront applications (`apps/storefront` and headless frontends).

pub mod core;
pub mod model;
pub mod render;
pub mod transport;
pub mod ui;

pub use model::{
    SeoAlternateLink, SeoDocument, SeoFieldSource, SeoFieldState, SeoImageAsset, SeoLinkTag,
    SeoMetaTag, SeoOpenGraph, SeoPageContext, SeoPagination, SeoRedirectDecision, SeoRobots,
    SeoRouteContext, SeoSchemaBlockKind, SeoStructuredDataBlock, SeoTwitterCard, SeoVerification,
    SeoVerificationTag,
};
pub use render::{render_head_html, render_head_html_with_nonce};
pub use transport::{SEO_PAGE_CONTEXT_QUERY, SeoPageContextResponse, SeoPageContextVariables};
pub use ui::SeoView;

#[cfg(test)]
mod contract_tests;
