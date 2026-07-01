use std::sync::Arc;

use async_trait::async_trait;
use rustok_search::graphql::{
    SearchGraphqlRateLimitError, SearchGraphqlRateLimitExceeded, SearchGraphqlRateLimiter,
    SearchGraphqlRateLimiterHandle,
};

use crate::middleware::rate_limit::{RateLimitCheckError, RateLimiter, SharedSearchRateLimiter};

struct ServerSearchGraphqlRateLimiter {
    limiter: Arc<RateLimiter>,
}

#[async_trait]
impl SearchGraphqlRateLimiter for ServerSearchGraphqlRateLimiter {
    fn namespace(&self) -> &str {
        self.limiter.namespace()
    }

    async fn check_rate_limit(&self, key: &str) -> Result<(), SearchGraphqlRateLimitError> {
        self.limiter
            .check_rate_limit(key)
            .await
            .map(|_| ())
            .map_err(|error| match error {
                RateLimitCheckError::Exceeded(exceeded) => {
                    SearchGraphqlRateLimitError::Exceeded(SearchGraphqlRateLimitExceeded {
                        limit: exceeded.limit,
                        retry_after: exceeded.retry_after,
                    })
                }
                RateLimitCheckError::BackendUnavailable(reason) => {
                    SearchGraphqlRateLimitError::BackendUnavailable(reason)
                }
            })
    }
}

pub fn search_graphql_rate_limiter_from_context(
    ctx: &loco_rs::app::AppContext,
) -> Option<SearchGraphqlRateLimiterHandle> {
    ctx.shared_store
        .get::<SharedSearchRateLimiter>()
        .map(|shared| {
            SearchGraphqlRateLimiterHandle(Arc::new(ServerSearchGraphqlRateLimiter {
                limiter: shared.0,
            }))
        })
}
