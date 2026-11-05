//! # rustok-seo-storefront
//!
//! Storefront and client UI support for RusToK SEO module.
//! Provides framework-agnostic SEO data structures, dual-path transport adapters,
//! and SSR head rendering helpers for Rust storefront hosts.

pub mod core;
pub mod model;
pub mod render;
pub mod transport;
pub mod ui;

pub use crate::model::{
    SeoAlternateLink, SeoDocument, SeoFieldSource, SeoFieldState, SeoImageAsset, SeoLinkTag,
    SeoMetaTag, SeoOpenGraph, SeoPageContext, SeoPagination, SeoRedirectDecision, SeoRobots,
    SeoRouteContext, SeoSchemaBlockKind, SeoStructuredDataBlock, SeoTwitterCard, SeoVerification,
    SeoVerificationTag,
};
pub use crate::render::{render_head_html, render_head_html_with_nonce};
pub use crate::transport::{
    SEO_PAGE_CONTEXT_QUERY, SeoPageContextResponse, SeoPageContextVariables,
};
pub use crate::ui::SeoView;

#[cfg(test)]
mod contract_tests;
