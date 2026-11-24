//! # rustok-content-orchestration
//!
//! Owner-owned cross-module content orchestration bridge for blog/forum/content workflows.
//!
//! Provides the runtime bridge implementation for content conversion workflows that span
//! `rustok-content`, `rustok-blog`, `rustok-forum`, `rustok-comments`, and `rustok-taxonomy`.
//! Source layout follows the canonical native module responsibilities: runtime handle wiring,
//! conversion bridge domain logic, and owner GraphQL mutations.

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
pub mod graphql;

pub(crate) mod errors;
pub mod runtime;

#[cfg(feature = "mod-blog")]
mod blog_taxonomy;
#[cfg(feature = "mod-blog")]
pub use blog_taxonomy::{blog_category_delete_cleanup, in_process_blog_taxonomy_port};

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
mod bridge;
#[cfg(all(feature = "mod-content", feature = "mod-blog", feature = "mod-forum"))]
mod route_resolver;

pub use runtime::{SharedContentOrchestrationService, build_content_orchestration_service};

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
pub use runtime::content_orchestration_from_shared;

#[cfg(all(
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
pub use bridge::ServerContentOrchestrationBridge;
#[cfg(all(feature = "mod-content", feature = "mod-blog", feature = "mod-forum"))]
pub use route_resolver::OwnerCanonicalRouteResolver;

#[cfg(all(
    test,
    feature = "mod-content",
    feature = "mod-blog",
    feature = "mod-forum",
    feature = "mod-comments"
))]
mod tests;

#[cfg(test)]
mod contract_tests;
