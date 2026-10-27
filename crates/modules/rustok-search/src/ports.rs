use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_core::Error;
use uuid::Uuid;

use crate::{
    PgSearchEngine, SearchEngine, SearchQuery, SearchResult, SearchSuggestion,
    SearchSuggestionQuery, SearchSuggestionService,
};

/// Raw Search owner boundary for trusted admin/host consumers only. This port
/// does not resolve a storefront channel and does not compose the declared
/// audience classes, so it can read gated documents. Never expose it directly
/// through a public search transport; public requests must use
/// `execute_audience_eligible_search` instead.
#[async_trait]
pub trait SearchQueryPort: Send + Sync {
    async fn execute_search(
        &self,
        context: PortContext,
        request: SearchQuery,
    ) -> Result<SearchResult, PortError>;
}

#[async_trait]
impl SearchQueryPort for PgSearchEngine {
    async fn execute_search(
        &self,
        context: PortContext,
        mut request: SearchQuery,
    ) -> Result<SearchResult, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let context_tenant_id = Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
            PortError::validation(
                "search.tenant_id_invalid",
                "search port context contains an invalid tenant identifier",
            )
        })?;
        if context_tenant_id.is_nil() {
            return Err(PortError::validation(
                "search.tenant_id_invalid",
                "search port context contains an invalid tenant identifier",
            ));
        }
        if let Some(request_tenant_id) = request.tenant_id
            && request_tenant_id != context_tenant_id
        {
            return Err(PortError::forbidden(
                "search.tenant_scope_mismatch",
                "search request tenant does not match the authoritative port context",
            ));
        }
        request.tenant_id = Some(context_tenant_id);
        request.locale.get_or_insert_with(|| context.locale.clone());
        self.search(request)
            .await
            .map_err(search_error_to_port_error)
    }
}

/// Transport-neutral owner boundary for autocomplete suggestions.
/// Raw suggestions for trusted admin/host consumers only; a storefront must use
/// `SearchSuggestionService::storefront_suggestions` with a resolved channel and a
/// `StorefrontSuggestionAudience`, because raw suggestions bypass audience classes.
#[async_trait]
pub trait SearchSuggestionPort: Send + Sync {
    async fn suggest(
        &self,
        context: PortContext,
        request: SearchSuggestionQuery,
    ) -> Result<Vec<SearchSuggestion>, PortError>;
}

#[async_trait]
impl SearchSuggestionPort for PgSearchEngine {
    async fn suggest(
        &self,
        context: PortContext,
        mut request: SearchSuggestionQuery,
    ) -> Result<Vec<SearchSuggestion>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        let context_tenant_id = Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
            PortError::validation(
                "search.tenant_id_invalid",
                "search port context contains an invalid tenant identifier",
            )
        })?;
        if context_tenant_id.is_nil() {
            return Err(PortError::validation(
                "search.tenant_id_invalid",
                "search port context contains an invalid tenant identifier",
            ));
        }
        if request.tenant_id != context_tenant_id {
            return Err(PortError::forbidden(
                "search.tenant_scope_mismatch",
                "search request tenant does not match the authoritative port context",
            ));
        }
        request.tenant_id = context_tenant_id;
        request.locale.get_or_insert_with(|| context.locale.clone());
        SearchSuggestionService::suggestions(self.connection(), request)
            .await
            .map_err(search_error_to_port_error)
    }
}

fn search_error_to_port_error(error: Error) -> PortError {
    match error {
        Error::Validation(message) => PortError::validation("search.validation", message),
        Error::NotFound(message) => PortError::new(
            rustok_api::PortErrorKind::NotFound,
            "search.not_found",
            message,
            false,
        ),
        Error::External(message) => PortError::unavailable("search.external", message),
        other => PortError::unavailable("search.unavailable", other.to_string()),
    }
}
