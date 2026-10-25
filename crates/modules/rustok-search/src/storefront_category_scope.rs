use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{AuthContext, PortError, RequestContext};
use uuid::Uuid;

pub const FORUM_SEARCH_SOURCE_MODULE: &str = "forum";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorefrontSearchTransport {
    Graphql,
    NativeServer,
}

#[derive(Clone)]
pub struct StorefrontSearchCategoryScopeRequest {
    pub tenant_id: Uuid,
    pub locale: String,
    pub fallback_locale: Option<String>,
    pub source_modules: Vec<String>,
    pub category_ids: Vec<Uuid>,
    pub auth: Option<AuthContext>,
    pub request_context: Option<RequestContext>,
    pub transport: StorefrontSearchTransport,
}

#[async_trait]
pub trait StorefrontSearchCategoryScopePort: Send + Sync {
    /// Source module whose storefront category tree this port owns.
    ///
    /// Search never guesses a category tree: it consults the capability only when the
    /// resolved request scope is exactly this module, so a new owner registers its own
    /// port instead of changing the executor.
    fn source_module(&self) -> &'static str;

    /// Expands the requested category roots into the exact document category scope.
    async fn expand_category_scope(
        &self,
        request: StorefrontSearchCategoryScopeRequest,
    ) -> Result<Vec<Uuid>, PortError>;
}

pub type SharedStorefrontSearchCategoryScopePort = Arc<dyn StorefrontSearchCategoryScopePort>;

/// True when the requested source-module scope is exactly the declaring module.
///
/// Both the resolver and every owner port use this predicate, so "the scope belongs to
/// this capability" has one definition instead of one per module.
pub fn scope_is_exactly(source_modules: &[String], module: &str) -> bool {
    source_modules.len() == 1 && source_modules[0] == module
}

/// Applies the owner category-scope capability to a single-module Search request.
///
/// The owner declares the source module it owns, so mixed, unspecified and unrelated
/// scopes retain exact-category semantics and Search never expands a tree by itself.
/// A host that did not compose the capability (module disabled or absent) also keeps
/// exact semantics: its documents are governed by the audience classes, not by this
/// optional capability.
pub async fn resolve_storefront_search_category_ids(
    port: Option<SharedStorefrontSearchCategoryScopePort>,
    request: StorefrontSearchCategoryScopeRequest,
) -> Result<Vec<Uuid>, PortError> {
    if request.category_ids.is_empty() {
        return Ok(request.category_ids);
    }
    let Some(port) = port else {
        return Ok(request.category_ids);
    };
    if !scope_is_exactly(&request.source_modules, port.source_module()) {
        return Ok(request.category_ids);
    }

    port.expand_category_scope(request).await
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::*;

    struct CountingScopePort {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl StorefrontSearchCategoryScopePort for CountingScopePort {
        fn source_module(&self) -> &'static str {
            FORUM_SEARCH_SOURCE_MODULE
        }

        async fn expand_category_scope(
            &self,
            request: StorefrontSearchCategoryScopeRequest,
        ) -> Result<Vec<Uuid>, PortError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(request.category_ids)
        }
    }

    fn request(source_modules: Vec<&str>) -> StorefrontSearchCategoryScopeRequest {
        StorefrontSearchCategoryScopeRequest {
            tenant_id: Uuid::new_v4(),
            locale: "en".to_string(),
            fallback_locale: Some("en".to_string()),
            source_modules: source_modules.into_iter().map(str::to_string).collect(),
            category_ids: vec![Uuid::new_v4()],
            auth: None,
            request_context: None,
            transport: StorefrontSearchTransport::Graphql,
        }
    }

    #[tokio::test]
    async fn declaring_module_scope_invokes_owner_port() {
        let calls = Arc::new(AtomicUsize::new(0));
        let port: SharedStorefrontSearchCategoryScopePort = Arc::new(CountingScopePort {
            calls: calls.clone(),
        });

        resolve_storefront_search_category_ids(Some(port), request(vec!["forum"]))
            .await
            .expect("Forum-only scope should resolve");

        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn absent_owner_capability_keeps_exact_categories() {
        // A host without the capability (module disabled or absent) must not fail a
        // public search: its documents are governed by the declared audience classes.
        let request = request(vec!["forum"]);
        let expected = request.category_ids.clone();

        let resolved = resolve_storefront_search_category_ids(None, request)
            .await
            .expect("absent owner capability keeps exact semantics");

        assert_eq!(resolved, expected);
    }

    #[tokio::test]
    async fn unrelated_single_module_scope_never_uses_another_owners_tree() {
        let calls = Arc::new(AtomicUsize::new(0));
        let port: SharedStorefrontSearchCategoryScopePort = Arc::new(CountingScopePort {
            calls: calls.clone(),
        });
        let request = request(vec!["blog"]);
        let expected = request.category_ids.clone();

        let resolved = resolve_storefront_search_category_ids(Some(port), request)
            .await
            .expect("unrelated scope remains exact");

        assert_eq!(resolved, expected);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn only_the_declaring_module_is_the_owning_scope() {
        assert!(scope_is_exactly(&["forum".to_string()], "forum"));
        assert!(!scope_is_exactly(&[], "forum"));
        assert!(!scope_is_exactly(
            &["forum".to_string(), "product".to_string()],
            "forum",
        ));
        assert!(!scope_is_exactly(&["product".to_string()], "forum"));
        assert!(!scope_is_exactly(&["forum".to_string()], "blog"));
    }

    #[tokio::test]
    async fn mixed_scope_preserves_exact_categories_without_owner_call() {
        let calls = Arc::new(AtomicUsize::new(0));
        let port: SharedStorefrontSearchCategoryScopePort = Arc::new(CountingScopePort {
            calls: calls.clone(),
        });
        let request = request(vec!["forum", "product"]);
        let expected = request.category_ids.clone();

        let resolved = resolve_storefront_search_category_ids(Some(port), request)
            .await
            .expect("mixed scope should remain exact");

        assert_eq!(resolved, expected);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}
