//! Runtime helpers for Redis-dependent features.

/// Resolve the Redis URL from environment variables.
///
/// Priority:
/// 1. `RUSTOK_REDIS_URL`
/// 2. `REDIS_URL`
///
/// Returns `None` when both values are absent or blank.
pub fn resolve_redis_url() -> Option<String> {
    std::env::var("RUSTOK_REDIS_URL")
        .ok()
        .or_else(|| std::env::var("REDIS_URL").ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::resolve_redis_url;

    fn clear_env() {
        unsafe {
            std::env::remove_var("RUSTOK_REDIS_URL");
            std::env::remove_var("REDIS_URL");
        }
    }

    #[test]
    fn prefers_rustok_redis_url_over_redis_url() {
        clear_env();
        unsafe {
            std::env::set_var("RUSTOK_REDIS_URL", "redis://primary:6379/0");
            std::env::set_var("REDIS_URL", "redis://fallback:6379/0");
        }

        assert_eq!(
            resolve_redis_url().as_deref(),
            Some("redis://primary:6379/0")
        );

        clear_env();
    }

    #[test]
    fn returns_none_for_blank_values() {
        clear_env();
        unsafe {
            std::env::set_var("REDIS_URL", "   ");
        }

        assert_eq!(resolve_redis_url(), None);

        clear_env();
    }
}
