use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_core::Error;

use crate::{
    PgSearchEngine, SearchEngine, SearchQuery, SearchResult, SearchSuggestion,
    SearchSuggestionQuery,
};

/// Transport-neutral owner boundary for search execution and suggestions.
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
        context.require_deadline_semantics()?;
        request.locale.get_or_insert_with(|| context.locale.clone());
        self.search(request)
            .await
            .map_err(search_error_to_port_error)
    }
}

/// Transport-neutral owner boundary for autocomplete suggestions.
#[async_trait]
pub trait SearchSuggestionPort: Send + Sync {
    async fn suggest(
        &self,
        context: PortContext,
        request: SearchSuggestionQuery,
    ) -> Result<Vec<SearchSuggestion>, PortError>;
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
