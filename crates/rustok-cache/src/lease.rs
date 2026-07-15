use std::time::{Duration, Instant};

use sha2::{Digest, Sha256};

use crate::CacheService;

const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(5);
const DEFAULT_LEASE_OPERATION_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_LEASE_TTL: Duration = Duration::from_secs(300);
const MAX_LEASE_CACHE_KEY_BYTES: usize = 16 * 1024;

#[cfg(feature = "redis-cache")]
const RELEASE_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('DEL', KEYS[1])
end
return 0
"#;

#[cfg(feature = "redis-cache")]
const EXTEND_SCRIPT: &str = r#"
if redis.call('GET', KEYS[1]) == ARGV[1] then
    return redis.call('PEXPIRE', KEYS[1], ARGV[2])
end
return 0
"#;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CacheLeaseOptions {
    pub ttl: Duration,
    pub operation_timeout: Duration,
}

impl Default for CacheLeaseOptions {
    fn default() -> Self {
        Self {
            ttl: DEFAULT_LEASE_TTL,
            operation_timeout: DEFAULT_LEASE_OPERATION_TIMEOUT,
        }
    }
}

impl CacheLeaseOptions {
    pub fn new(ttl: Duration, operation_timeout: Duration) -> Result<Self, CacheLeaseError> {
        let options = Self {
            ttl,
            operation_timeout,
        };
        options.validate()?;
        Ok(options)
    }

    fn validate(&self) -> Result<(), CacheLeaseError> {
        validate_ttl(self.ttl)?;
        validate_operation_timeout(self.operation_timeout, self.ttl)
    }
}

pub enum CacheLeaseOutcome {
    Acquired(Box<DistributedCacheLease>),
    Contended,
}

impl std::fmt::Debug for CacheLeaseOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Acquired(lease) => formatter.debug_tuple("Acquired").field(lease).finish(),
            Self::Contended => formatter.write_str("Contended"),
        }
    }
}

/// Token-owned Redis lease for cross-instance hot-key load coordination.
///
/// `release` and `extend` use compare-and-delete/expire Lua scripts. A process can never
/// delete or extend a lease that expired and was acquired by another owner. Dropping the
/// value does not issue asynchronous I/O; TTL expiration remains the final safety net.
pub struct DistributedCacheLease {
    #[cfg(feature = "redis-cache")]
    client: redis::Client,
    key: String,
    token: String,
    ttl: Duration,
    operation_timeout: Duration,
    expires_at: Instant,
}

impl std::fmt::Debug for DistributedCacheLease {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DistributedCacheLease")
            .field("key", &self.key)
            .field("ttl", &self.ttl)
            .field("remaining_ttl", &self.remaining_ttl())
            .field("operation_timeout", &self.operation_timeout)
            .finish_non_exhaustive()
    }
}

impl DistributedCacheLease {
    pub fn key(&self) -> &str {
        &self.key
    }

    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Conservative process-local estimate. The deadline starts before the Redis SET/PEXPIRE
    /// response is received, so it never overstates the usable server lease duration.
    pub fn remaining_ttl(&self) -> Duration {
        self.expires_at.saturating_duration_since(Instant::now())
    }

    pub fn is_expired(&self) -> bool {
        self.remaining_ttl().is_zero()
    }

    /// Extend the lease only while this instance still owns the token.
    pub async fn extend(&mut self, ttl: Duration) -> Result<bool, CacheLeaseError> {
        validate_ttl(ttl)?;
        validate_operation_timeout(self.operation_timeout, ttl)?;

        #[cfg(feature = "redis-cache")]
        {
            let mut connection = redis_operation(
                self.operation_timeout,
                "lease extend connection",
                self.client.get_multiplexed_async_connection(),
            )
            .await?;
            let ttl_millis = duration_millis_ceil(ttl);
            let started_at = Instant::now();
            let extended = redis_operation(
                self.operation_timeout,
                "lease extend",
                redis::Script::new(EXTEND_SCRIPT)
                    .key(&self.key)
                    .arg(&self.token)
                    .arg(ttl_millis)
                    .invoke_async::<i64>(&mut connection),
            )
            .await?
                == 1;
            if extended {
                let expires_at = started_at
                    .checked_add(ttl)
                    .ok_or(CacheLeaseError::DeadlineOverflow)?;
                if Instant::now() >= expires_at {
                    return Err(CacheLeaseError::ExpiredBeforeConfirmation);
                }
                self.ttl = ttl;
                self.expires_at = expires_at;
            }
            Ok(extended)
        }

        #[cfg(not(feature = "redis-cache"))]
        {
            let _ = ttl;
            Err(CacheLeaseError::RedisNotConfigured)
        }
    }

    /// Release the lease only if its ownership token still matches Redis.
    pub async fn release(self) -> Result<bool, CacheLeaseError> {
        #[cfg(feature = "redis-cache")]
        {
            let mut connection = redis_operation(
                self.operation_timeout,
                "lease release connection",
                self.client.get_multiplexed_async_connection(),
            )
            .await?;
            let released = redis_operation(
                self.operation_timeout,
                "lease release",
                redis::Script::new(RELEASE_SCRIPT)
                    .key(&self.key)
                    .arg(&self.token)
                    .invoke_async::<i64>(&mut connection),
            )
            .await?
                == 1;
            Ok(released)
        }

        #[cfg(not(feature = "redis-cache"))]
        {
            Err(CacheLeaseError::RedisNotConfigured)
        }
    }
}

impl CacheService {
    /// Attempt to acquire a token-safe distributed lease for a cache key.
    ///
    /// The original cache key is SHA-256 hashed before becoming part of the Redis lease key,
    /// keeping lock keys bounded and avoiding disclosure of tenant/user identifiers.
    pub async fn try_acquire_distributed_lease(
        &self,
        scope: &str,
        cache_key: &str,
        options: CacheLeaseOptions,
    ) -> Result<CacheLeaseOutcome, CacheLeaseError> {
        options.validate()?;
        let key = lease_key(scope, cache_key)?;

        #[cfg(feature = "redis-cache")]
        {
            let client = self
                .redis_client()
                .cloned()
                .ok_or(CacheLeaseError::RedisNotConfigured)?;
            let token = uuid::Uuid::new_v4().to_string();
            let mut connection = redis_operation(
                options.operation_timeout,
                "lease acquire connection",
                client.get_multiplexed_async_connection(),
            )
            .await?;
            let started_at = Instant::now();
            let response = redis_operation(
                options.operation_timeout,
                "lease acquire",
                redis::cmd("SET")
                    .arg(&key)
                    .arg(&token)
                    .arg("NX")
                    .arg("PX")
                    .arg(duration_millis_ceil(options.ttl))
                    .query_async::<Option<String>>(&mut connection),
            )
            .await?;

            if response.is_some() {
                let expires_at = started_at
                    .checked_add(options.ttl)
                    .ok_or(CacheLeaseError::DeadlineOverflow)?;
                if Instant::now() >= expires_at {
                    return Err(CacheLeaseError::ExpiredBeforeConfirmation);
                }
                Ok(CacheLeaseOutcome::Acquired(Box::new(
                    DistributedCacheLease {
                        client,
                        key,
                        token,
                        ttl: options.ttl,
                        operation_timeout: options.operation_timeout,
                        expires_at,
                    },
                )))
            } else {
                Ok(CacheLeaseOutcome::Contended)
            }
        }

        #[cfg(not(feature = "redis-cache"))]
        {
            let _ = key;
            Err(CacheLeaseError::RedisNotConfigured)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CacheLeaseError {
    RedisNotConfigured,
    EmptyScope,
    InvalidScope(String),
    EmptyCacheKey,
    CacheKeyTooLarge {
        length: usize,
        maximum: usize,
    },
    ZeroTtl,
    TtlTooLarge {
        maximum_seconds: u64,
    },
    ZeroOperationTimeout,
    OperationTimeoutNotLessThanTtl {
        timeout_ms: u128,
        ttl_ms: u128,
    },
    DeadlineOverflow,
    ExpiredBeforeConfirmation,
    Timeout {
        operation: &'static str,
        millis: u128,
    },
    Redis {
        operation: &'static str,
        message: String,
    },
}

impl std::fmt::Display for CacheLeaseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RedisNotConfigured => write!(formatter, "Redis cache lease is not configured"),
            Self::EmptyScope => write!(formatter, "cache lease scope must not be empty"),
            Self::InvalidScope(scope) => write!(
                formatter,
                "cache lease scope contains unsupported characters: {scope:?}"
            ),
            Self::EmptyCacheKey => write!(formatter, "cache lease key must not be empty"),
            Self::CacheKeyTooLarge { length, maximum } => write!(
                formatter,
                "cache lease source key is {length} bytes; maximum is {maximum}"
            ),
            Self::ZeroTtl => write!(formatter, "cache lease TTL must be greater than zero"),
            Self::TtlTooLarge { maximum_seconds } => write!(
                formatter,
                "cache lease TTL exceeds maximum {maximum_seconds} seconds"
            ),
            Self::ZeroOperationTimeout => {
                write!(
                    formatter,
                    "cache lease operation timeout must be greater than zero"
                )
            }
            Self::OperationTimeoutNotLessThanTtl { timeout_ms, ttl_ms } => write!(
                formatter,
                "cache lease operation timeout {timeout_ms} ms must be less than TTL {ttl_ms} ms"
            ),
            Self::DeadlineOverflow => write!(formatter, "cache lease deadline overflowed"),
            Self::ExpiredBeforeConfirmation => write!(
                formatter,
                "cache lease expired before Redis ownership confirmation was received"
            ),
            Self::Timeout { operation, millis } => {
                write!(formatter, "cache {operation} timed out after {millis} ms")
            }
            Self::Redis { operation, message } => {
                write!(formatter, "cache {operation} Redis error: {message}")
            }
        }
    }
}

impl std::error::Error for CacheLeaseError {}

fn lease_key(scope: &str, cache_key: &str) -> Result<String, CacheLeaseError> {
    if scope.is_empty() {
        return Err(CacheLeaseError::EmptyScope);
    }
    if scope.len() > 64
        || !scope
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b'.'))
    {
        return Err(CacheLeaseError::InvalidScope(scope.to_string()));
    }
    if cache_key.trim().is_empty() {
        return Err(CacheLeaseError::EmptyCacheKey);
    }
    if cache_key.len() > MAX_LEASE_CACHE_KEY_BYTES {
        return Err(CacheLeaseError::CacheKeyTooLarge {
            length: cache_key.len(),
            maximum: MAX_LEASE_CACHE_KEY_BYTES,
        });
    }

    Ok(format!(
        "rustok:cache-lease:v1:{scope}:{}",
        hex::encode(Sha256::digest(cache_key.as_bytes()))
    ))
}

fn validate_ttl(ttl: Duration) -> Result<(), CacheLeaseError> {
    if ttl.is_zero() {
        return Err(CacheLeaseError::ZeroTtl);
    }
    if ttl > MAX_LEASE_TTL {
        return Err(CacheLeaseError::TtlTooLarge {
            maximum_seconds: MAX_LEASE_TTL.as_secs(),
        });
    }
    Ok(())
}

fn validate_operation_timeout(
    operation_timeout: Duration,
    ttl: Duration,
) -> Result<(), CacheLeaseError> {
    if operation_timeout.is_zero() {
        return Err(CacheLeaseError::ZeroOperationTimeout);
    }
    if operation_timeout >= ttl {
        return Err(CacheLeaseError::OperationTimeoutNotLessThanTtl {
            timeout_ms: duration_millis_ceil(operation_timeout) as u128,
            ttl_ms: duration_millis_ceil(ttl) as u128,
        });
    }
    Ok(())
}

fn duration_millis_ceil(duration: Duration) -> u64 {
    let millis = duration.as_millis();
    if millis == 0 {
        1
    } else {
        millis.min(u128::from(u64::MAX)) as u64
    }
}

#[cfg(feature = "redis-cache")]
async fn redis_operation<T, E, F>(
    timeout: Duration,
    operation: &'static str,
    future: F,
) -> Result<T, CacheLeaseError>
where
    F: std::future::Future<Output = Result<T, E>>,
    E: std::fmt::Display,
{
    tokio::time::timeout(timeout, future)
        .await
        .map_err(|_| CacheLeaseError::Timeout {
            operation,
            millis: timeout.as_millis(),
        })?
        .map_err(|error| CacheLeaseError::Redis {
            operation,
            message: error.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lease_key_is_bounded_stable_and_redacts_original_key() {
        let original = "rustok:prod:tenant-secret:catalog:v1:product:42";
        let first = lease_key("catalog-refresh", original).unwrap();
        let second = lease_key("catalog-refresh", original).unwrap();

        assert_eq!(first, second);
        assert!(!first.contains("tenant-secret"));
        assert!(first.len() < 128);
    }

    #[test]
    fn validates_lease_options() {
        assert_eq!(
            CacheLeaseOptions::new(Duration::ZERO, Duration::from_millis(1)).unwrap_err(),
            CacheLeaseError::ZeroTtl
        );
        assert_eq!(
            CacheLeaseOptions::new(Duration::from_secs(1), Duration::ZERO).unwrap_err(),
            CacheLeaseError::ZeroOperationTimeout
        );
        assert_eq!(
            CacheLeaseOptions::new(Duration::from_secs(301), Duration::from_secs(1)).unwrap_err(),
            CacheLeaseError::TtlTooLarge {
                maximum_seconds: 300,
            }
        );
        assert_eq!(
            CacheLeaseOptions::new(Duration::from_millis(100), Duration::from_millis(100))
                .unwrap_err(),
            CacheLeaseError::OperationTimeoutNotLessThanTtl {
                timeout_ms: 100,
                ttl_ms: 100,
            }
        );
    }

    #[test]
    fn validates_source_cache_key_before_hashing() {
        assert_eq!(
            lease_key("catalog", "   ").unwrap_err(),
            CacheLeaseError::EmptyCacheKey
        );
        assert!(matches!(
            lease_key("catalog", &"k".repeat(MAX_LEASE_CACHE_KEY_BYTES + 1)).unwrap_err(),
            CacheLeaseError::CacheKeyTooLarge { .. }
        ));
    }

    #[test]
    fn positive_sub_millisecond_ttl_rounds_up() {
        assert_eq!(duration_millis_ceil(Duration::from_nanos(1)), 1);
    }

    #[cfg(feature = "redis-cache")]
    #[tokio::test]
    async fn acquisition_reports_unconfigured_redis_before_io() {
        let service = CacheService::from_url(Some("not-a-valid-redis-url"));
        let error = service
            .try_acquire_distributed_lease("catalog", "key", CacheLeaseOptions::default())
            .await
            .unwrap_err();
        assert_eq!(error, CacheLeaseError::RedisNotConfigured);
    }
}
