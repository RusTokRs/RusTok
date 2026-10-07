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
fn tenant_listener_cleanup_preserves_the_generic_task_slot() {
    let middleware = source("apps/server/src/middleware/mod.rs");
    let runtime = source("apps/server/src/services/server_runtime_context.rs");

    assert!(runtime.contains("pub fn shared_take<T>(&self) -> Option<T>"));
    assert!(runtime.contains(".remove(&TypeId::of::<T>())?"));
    assert!(middleware.contains("start_tenant_cache_generation_listener("));
    assert!(middleware.contains("start_tenant_locale_generation_listener("));
    assert!(
        !middleware.contains("shared_map::<tokio::task::JoinHandle<()>, _>(|task| task.abort())")
    );
}
