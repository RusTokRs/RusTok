/// Backend statistics.
///
/// Semantics are **aggregate and wrapper-inclusive**: a wrapper that instruments its own
/// operations reports the counters it observed *plus* whatever its inner backend reports, and
/// `entries` is a best-effort estimate (`max` for composite backends). Treat these values as
/// directional signals, not as exact, non-overlapping event counts.
#[derive(Debug, Clone, Copy, Default)]
pub struct CacheStats {
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
    pub entries: u64,
}
