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
        BlogGraphqlRateLimiterHandle(Arc::new(ServerBlogGraphqlRateLimiter { limiter: shared.0 }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::rate_limit::RateLimitConfig;

    #[tokio::test]
    async fn memory_backend_maps_exceeded_result_into_blog_contract() {
        let limiter = Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::new(1, 30),
            "blog-graphql-adapter-test",
        ));
        let adapter = ServerBlogGraphqlRateLimiter { limiter };
        let key = "tenant:test:blog:graphql:read:posts:anonymous";

        assert_eq!(adapter.namespace(), "blog-graphql-adapter-test");
        assert!(adapter.check_rate_limit(key).await.is_ok());

        let error = adapter
            .check_rate_limit(key)
            .await
            .expect_err("second request must exceed the one-request window");
        match error {
            BlogGraphqlRateLimitError::Exceeded(exceeded) => {
                assert_eq!(exceeded.limit, 1);
                assert!(exceeded.retry_after > 0);
                assert!(exceeded.retry_after <= 30);
            }
            other => panic!("expected exceeded error, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn disabled_memory_backend_remains_unlimited_through_adapter() {
        let limiter = Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::disabled(),
            "blog-graphql-disabled-test",
        ));
        let adapter = ServerBlogGraphqlRateLimiter { limiter };
        let key = "tenant:test:blog:graphql:write:create_post:user:test";

        for _ in 0..3 {
            assert!(adapter.check_rate_limit(key).await.is_ok());
        }
    }
}
