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
fn event_ids_use_the_ulid_2_generation_api() {
    let core_ids = source("crates/rustok-core/src/id.rs");
    let events = source("crates/rustok-events/src/types.rs");

    for source in [&core_ids, &events] {
        assert!(source.contains("Ulid::gen()"));
        assert!(!source.contains("Ulid::new()"));
    }
}

#[test]
fn inventory_expressions_and_retry_ownership_match_current_apis() {
    let inventory = source("crates/rustok-inventory/src/ports.rs");
    let events = source("crates/rustok-events/src/types.rs");

    assert!(inventory.contains("sea_query::{Expr, ExprTrait}"));
    assert_eq!(inventory.matches("Expr::current_timestamp().into()").count(), 2);
    assert!(inventory.contains("request.metadata.clone()"));
    assert!(events.contains("size_bytes: _,"));
}
