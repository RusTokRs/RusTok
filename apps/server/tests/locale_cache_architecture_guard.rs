use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/server should live under workspace root")
        .to_path_buf()
}

fn source(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
}

#[test]
fn tenant_locale_cache_is_weighted_coalesced_and_registered_atomically() {
    let locale = source("apps/server/src/middleware/locale.rs");
    for required in [
        ".weigher(tenant_locale_entry_weight)",
        ".try_get_with(tenant_id",
        "let candidate = Arc::new(TenantLocaleCache::new());",
        "ctx.shared_insert_if_absent(candidate.clone())",
        "ctx.shared_get::<Arc<TenantLocaleCache>>()",
        ".unwrap_or(candidate)",
    ] {
        assert!(
            locale.contains(required),
            "tenant locale cache must retain {required}"
        );
    }
    assert!(!locale.contains("ctx.shared_insert(cache.clone());"));
}
