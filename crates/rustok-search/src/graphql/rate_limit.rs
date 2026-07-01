use async_trait::async_trait;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchGraphqlRateLimitExceeded {
    pub limit: usize,
    pub retry_after: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchGraphqlRateLimitError {
    Exceeded(SearchGraphqlRateLimitExceeded),
    BackendUnavailable(String),
}

#[async_trait]
pub trait SearchGraphqlRateLimiter: Send + Sync {
    fn namespace(&self) -> &str;

    async fn check_rate_limit(&self, key: &str) -> Result<(), SearchGraphqlRateLimitError>;
}

#[derive(Clone)]
pub struct SearchGraphqlRateLimiterHandle(pub Arc<dyn SearchGraphqlRateLimiter>);
