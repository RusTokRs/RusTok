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
fn production_bootstrap_uses_the_hardened_registry_provider() {
    let runtime = source("apps/server/src/services/app_runtime.rs");
    assert!(
        runtime.contains("HardenedRegistryMarketplaceProvider::from_env()"),
        "production bootstrap must use the bounded registry provider"
    );
    assert!(
        runtime.contains("LocalManifestMarketplaceProvider"),
        "local manifest must remain the first canonical provider"
    );
    assert!(
        !runtime.contains("MarketplaceCatalogService::evolutionary_defaults()"),
        "production bootstrap must not reactivate the legacy count-only registry cache"
    );
}

#[test]
fn marketplace_registry_cache_is_bounded_hashed_and_single_flight() {
    let provider = source("apps/server/src/services/marketplace_catalog_cache.rs");
    for required in [
        "DEFAULT_REGISTRY_CACHE_MAX_WEIGHT_BYTES",
        "DEFAULT_REGISTRY_RESPONSE_MAX_BYTES",
        "MAX_REGISTRY_CACHE_KEY_COMPONENT_BYTES",
        ".weigher(cached_registry_catalog_weight)",
        ".try_get_with(cache_key",
        "Sha256::new()",
        "read_bounded_response",
        "response.bytes_stream()",
        "cache_key_is_bounded_hashed_and_length_delimited",
        "concurrent_catalog_misses_are_single_flight",
        "chunked_oversized_response_is_rejected_before_json_parse",
    ] {
        assert!(
            provider.contains(required),
            "marketplace cache hardening must retain {required}"
        );
    }
    assert!(
        !provider.contains("format!(\"{}#{}\", registry_url"),
        "marketplace cache keys must not expose raw registry URL/query input"
    );
}

#[test]
fn marketplace_registry_response_streaming_is_enabled() {
    let manifest = source("apps/server/Cargo.toml");
    assert!(
        manifest.contains("\"rustls-tls\", \"stream\"")
            || manifest.contains("\"stream\", \"rustls-tls\""),
        "reqwest stream support is required to reject oversized chunked bodies before full allocation"
    );
}
