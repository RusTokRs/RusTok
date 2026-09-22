/// Timeout Helper
///
/// Enforce operation deadlines to prevent hanging requests.
use std::time::Duration;
use tokio::time::timeout;

/// Execute operation with timeout
pub async fn with_timeout<F, Fut, T>(duration: Duration, f: F) -> Result<T, TimeoutError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = T>,
{
    match timeout(duration, f()).await {
        Ok(result) => Ok(result),
        Err(_) => Err(TimeoutError { duration }),
    }
}

/// Timeout error
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[error("Operation timed out after {duration:?}")]
pub struct TimeoutError {
    pub duration: Duration,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_timeout_success() {
        let result = with_timeout(Duration::from_millis(100), || async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        })
        .await;

        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn test_timeout_exceeded() {
        let result = with_timeout(Duration::from_millis(10), || async {
            tokio::time::sleep(Duration::from_millis(100)).await;
            42
        })
        .await;

        assert!(result.is_err());
    }
}
