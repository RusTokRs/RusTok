/// Catalog of client-shipped admin persisted hashes observed in the current UI fleet.
///
/// This list is telemetry-only. It is not an authorization or allow/deny source of truth.
use std::collections::HashSet;

use once_cell::sync::Lazy;

pub static ADMIN_PERSISTED_QUERY_HASHES: Lazy<HashSet<&'static str>> = Lazy::new(|| {
    HashSet::from([
        // apps/admin/src/pages/users.rs (optimized list query)
        "ff1e132e28d2e1c804d8d5ade5966307e17685b9f4b39262d70ecaa4d49abb66",
        // apps/admin/src/pages/user_details.rs
        "85f7f7ba212ab47e951fcf7dbb30bb918e66b88710574a576b0088877653f3b7",
    ])
});

pub fn is_cataloged_admin_hash(hash: &str) -> bool {
    ADMIN_PERSISTED_QUERY_HASHES.contains(hash)
}
