//! Runtime helpers for Redis-dependent features.

/// Resolve the deployment Redis URL.
///
/// The resolver lives in the cache crate (`rustok_cache::resolve_redis_url`) so the runtime target
/// and the target the settings API reports come from the same place; this re-export keeps the host
/// call sites unchanged without maintaining a second copy of the precedence rules.
pub use rustok_cache::resolve_redis_url;

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
