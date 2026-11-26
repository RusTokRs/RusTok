//! Runtime helpers for Redis-dependent features.

/// Resolve the deployment Redis URL.
///
/// The resolver lives in the cache crate (`rustok_cache::resolve_redis_url`) so the runtime target
/// and the target the settings API reports come from the same place; this re-export keeps the host
/// call sites unchanged without maintaining a second copy of the precedence rules.
pub use rustok_cache::resolve_redis_url;
