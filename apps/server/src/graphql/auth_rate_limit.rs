use std::sync::Arc;

use async_trait::async_trait;
use rustok_auth::graphql::{
    AuthGraphqlRateLimitError, AuthGraphqlRateLimitExceeded, AuthGraphqlRateLimiter,
    AuthGraphqlRateLimiterHandle,
};

use crate::middleware::rate_limit::{RateLimitCheckError, RateLimiter, SharedAuthRateLimiter};
use crate::services::server_runtime_context::ServerRuntimeContext;

struct ServerAuthGraphqlRateLimiter {
    limiter: Arc<RateLimiter>,
}

#[async_trait]
impl AuthGraphqlRateLimiter for ServerAuthGraphqlRateLimiter {
    fn namespace(&self) -> &str {
        self.limiter.namespace()
    }

    async fn check_rate_limit(&self, key: &str) -> Result<(), AuthGraphqlRateLimitError> {
        self.limiter
            .check_rate_limit(key)
            .await
            .map(|_| ())
            .map_err(|error| match error {
                RateLimitCheckError::Exceeded(exceeded) => {
                    AuthGraphqlRateLimitError::Exceeded(AuthGraphqlRateLimitExceeded {
                        limit: exceeded.limit,
                        retry_after: exceeded.retry_after,
                    })
                }
                RateLimitCheckError::BackendUnavailable(reason) => {
                    AuthGraphqlRateLimitError::BackendUnavailable(reason)
                }
            })
    }
}

/// The dedicated auth limiter (the same namespace that throttles the REST
/// credential endpoints) also guards the GraphQL credential mutations, so
/// switching transports cannot bypass the stricter per-IP budget.
pub fn auth_graphql_rate_limiter_from_context(
    ctx: &ServerRuntimeContext,
) -> Option<AuthGraphqlRateLimiterHandle> {
    ctx.shared_get::<SharedAuthRateLimiter>().map(|shared| {
        AuthGraphqlRateLimiterHandle(Arc::new(ServerAuthGraphqlRateLimiter { limiter: shared.0 }))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::middleware::rate_limit::RateLimitConfig;

    #[tokio::test]
    async fn memory_backend_maps_exceeded_result_into_auth_contract() {
        let limiter = Arc::new(RateLimiter::new_with_namespace(
            RateLimitConfig::new(1, 30),
            "auth-graphql-adapter-test",
        ));
        let adapter = ServerAuthGraphqlRateLimiter { limiter };
        let key = "tenant:00000000-0000-0000-0000-000000000000:auth:graphql:sign_in:ip:203.0.113.7";

        adapter.check_rate_limit(key).await.unwrap();
        let error = adapter.check_rate_limit(key).await.unwrap_err();

        assert!(matches!(
            error,
            AuthGraphqlRateLimitError::Exceeded(AuthGraphqlRateLimitExceeded { limit: 1, .. })
        ));
    }
}
