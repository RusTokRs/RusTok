//! Port for public route resolution.
//!
//! `rustok-content` defines the contract and owns no route tables. Blog and
//! Forum own their routes and redirects, and `rustok-content-orchestration`
//! implements this port by dispatching on the route namespace. Consumers (SEO,
//! storefront, GraphQL) receive the port through injection and never query
//! owner tables directly.

use async_trait::async_trait;
use uuid::Uuid;

use crate::{ContentError, ContentResult};

/// Result of resolving a public route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedContentRoute {
    pub target_kind: String,
    pub target_id: Uuid,
    pub locale: String,
    pub matched_url: String,
    pub canonical_url: String,
    pub redirect_required: bool,
}

#[async_trait]
pub trait CanonicalRouteResolver: Send + Sync {
    /// Resolves `route` for `locale` within `tenant_id`.
    ///
    /// Returns `Ok(None)` when no owner recognises the route or the target is
    /// missing. Owner-table errors are returned as `Err` and never fall back to
    /// another owner.
    async fn resolve_route(
        &self,
        tenant_id: Uuid,
        locale: &str,
        route: &str,
    ) -> ContentResult<Option<ResolvedContentRoute>>;
}

/// Validates the shape shared by every public route key. Owners apply their own
/// namespace checks on top of this.
pub fn normalize_route(route: &str) -> ContentResult<String> {
    let route = route.trim();
    if route.is_empty() {
        return Err(ContentError::validation("route must not be empty"));
    }
    if route.len() > 512 {
        return Err(ContentError::validation("route must be <= 512 chars"));
    }
    if !route.starts_with('/') {
        return Err(ContentError::validation("route must start with `/`"));
    }
    if route.chars().any(char::is_whitespace) || route.contains("://") {
        return Err(ContentError::validation(
            "route must be a relative path without whitespace or scheme",
        ));
    }
    Ok(route.to_string())
}

/// Runtime-extension wrapper that carries the injected resolver to consumers
/// (SEO, storefront, GraphQL). Typed wrappers keep the lookup key unambiguous.
#[derive(Clone)]
pub struct SharedCanonicalRouteResolver(pub std::sync::Arc<dyn CanonicalRouteResolver>);
