use std::sync::Arc;

use async_trait::async_trait;
use rustok_blog::graphql::{
    BlogGraphqlRateLimitError, BlogGraphqlRateLimitExceeded, BlogGraphqlRateLimiter,
    BlogGraphqlRateLimiterHandle,
};

use crate::middleware::rate_limit::{RateLimitCheckError, RateLimiter, SharedApiRateLimiter};
use crate::services::server_runtime_context::ServerRuntimeContext;

struct ServerBlogGraphqlRateLimiter {
    limiter: Arc<RateLimiter>,
}

#[async_trait]
impl BlogGraphqlRateLimiter for ServerBlogGraphqlRateLimiter {
    fn namespace(&self) -> &str {
        self.limiter.namespace()
    }

    async fn check_rate_limit(&self, key: &str) -> Result<(), BlogGraphqlRateLimitError> {
        self.limiter
            .check_rate_limit(key)
            .await
            .map(|_| ())
            .map_err(|error| match error {
                RateLimitCheckError::Exceeded(exceeded) => {
                    BlogGraphqlRateLimitError::Exceeded(BlogGraphqlRateLimitExceeded {
                        limit: exceeded.limit,
                        retry_after: exceeded.retry_after,
                    })
                }
                RateLimitCheckError::BackendUnavailable(reason) => {
                    BlogGraphqlRateLimitError::BackendUnavailable(reason)
                }
            })
    }
}

pub fn blog_graphql_rate_limiter_from_context(
    ctx: &ServerRuntimeContext,
) -> Option<BlogGraphqlRateLimiterHandle> {
    ctx.shared_get::<SharedApiRateLimiter>().map(|shared| {
        BlogGraphqlRateLimiterHandle(Arc::new(ServerBlogGraphqlRateLimiter {
            limiter: shared.0,
        }))
    })
}
