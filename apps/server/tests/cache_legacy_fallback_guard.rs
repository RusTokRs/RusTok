use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("apps/server should live under workspace root")
        .to_path_buf()
}

fn collect_rust_files(root: &Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(root)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", root.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            collect_rust_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

#[test]
fn production_code_uses_only_the_canonical_degradation_aware_fallback() {
    let root = repo_root();
    let legacy_definition = root.join("crates/rustok-core/src/cache.rs");
    let legacy_reexport = root.join("crates/rustok-core/src/lib.rs");
    let this_guard = root.join("apps/server/tests/cache_legacy_fallback_guard.rs");
    let mut files = Vec::new();
    collect_rust_files(&root.join("apps"), &mut files);
    collect_rust_files(&root.join("crates"), &mut files);

    let mut violations = Vec::new();
    for path in files {
        if path == legacy_definition || path == legacy_reexport || path == this_guard {
            continue;
        }
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        if source.contains("FallbackCacheBackend") {
            violations.push(
                path.strip_prefix(&root)
                    .unwrap_or(&path)
                    .display()
                    .to_string(),
            );
        }
    }

    assert!(
        violations.is_empty(),
        "legacy rustok-core fallback must not gain production call sites: {violations:?}"
    );

    let canonical = std::fs::read_to_string(root.join("crates/rustok-cache/src/shared_backend.rs"))
        .expect("canonical shared cache backend source");
    assert!(canonical.contains("DegradationAwareFallbackBackend::new("));
}
