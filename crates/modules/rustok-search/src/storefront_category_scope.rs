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

impl StorefrontSearchCategoryScopeRequest {
    pub fn is_explicit_forum_only(&self) -> bool {
        self.source_modules.len() == 1 && self.source_modules[0] == FORUM_SEARCH_SOURCE_MODULE
    }
}

#[async_trait]
pub trait StorefrontSearchCategoryScopePort: Send + Sync {
    async fn expand_forum_category_scope(
        &self,
        request: StorefrontSearchCategoryScopeRequest,
    ) -> Result<Vec<Uuid>, PortError>;
}

pub type SharedStorefrontSearchCategoryScopePort = Arc<dyn StorefrontSearchCategoryScopePort>;

/// Applies the owner category-scope capability only to an explicit Forum-only
/// Search request. Mixed, unspecified, and non-Forum source scopes retain the
/// existing exact-category semantics. An explicit owner path fails closed when
/// its capability is not composed.
pub async fn resolve_storefront_search_category_ids(
    port: Option<SharedStorefrontSearchCategoryScopePort>,
    request: StorefrontSearchCategoryScopeRequest,
) -> Result<Vec<Uuid>, PortError> {
    if request.category_ids.is_empty() || !request.is_explicit_forum_only() {
        return Ok(request.category_ids);
    }

    let port = port.ok_or_else(|| {
        PortError::unavailable(
            "forum.search_category_scope.owner_unavailable",
            "Forum Search category scope is temporarily unavailable",
        )
    })?;

    port.expand_forum_category_scope(request).await
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
        async fn expand_forum_category_scope(
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
    async fn explicit_forum_only_scope_invokes_owner_port() {
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
    async fn explicit_forum_only_scope_requires_owner_port() {
        let error = resolve_storefront_search_category_ids(None, request(vec!["forum"]))
            .await
            .expect_err("explicit Forum scope must fail closed without its owner");

        assert_eq!(error.kind, rustok_api::PortErrorKind::Unavailable);
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
