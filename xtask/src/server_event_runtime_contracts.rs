use super::*;

pub(crate) fn validate_server_event_runtime_contract(manifest_path: &Path) -> Result<()> {
    let workspace_root = manifest_path.parent().with_context(|| {
        format!(
            "Failed to resolve workspace root from modules manifest {}",
            manifest_path.display()
        )
    })?;
    let services_dir = workspace_root
        .join("apps")
        .join("server")
        .join("src")
        .join("services");
    let app_runtime_path = services_dir.join("app_runtime.rs");
    let services_mod_path = services_dir.join("mod.rs");
    let module_dispatcher_path = services_dir.join("module_event_dispatcher.rs");
    let legacy_index_dispatcher_path = services_dir.join("index_dispatcher.rs");
    let legacy_search_dispatcher_path = services_dir.join("search_dispatcher.rs");

    let app_runtime = fs::read_to_string(&app_runtime_path)
        .with_context(|| format!("Failed to read {}", app_runtime_path.display()))?;
    let services_mod = fs::read_to_string(&services_mod_path)
        .with_context(|| format!("Failed to read {}", services_mod_path.display()))?;

    if !module_dispatcher_path.exists() {
        anyhow::bail!(
            "Server event runtime contract drift: {} is missing",
            module_dispatcher_path.display()
        );
    }
    if legacy_index_dispatcher_path.exists() {
        anyhow::bail!(
            "Server event runtime contract drift: legacy dispatcher {} must not exist",
            legacy_index_dispatcher_path.display()
        );
    }
    if legacy_search_dispatcher_path.exists() {
        anyhow::bail!(
            "Server event runtime contract drift: legacy dispatcher {} must not exist",
            legacy_search_dispatcher_path.display()
        );
    }
    let direct_dispatcher_export = services_mod.contains("pub mod module_event_dispatcher;");
    let dispatcher_facade_export = services_mod
        .contains("#[path = \"module_event_dispatcher.rs\"]")
        && services_mod.contains("mod module_event_dispatcher_base;")
        && services_mod.contains("pub mod module_event_dispatcher {")
        && services_mod.contains("pub use super::module_event_dispatcher_base::{");
    if !direct_dispatcher_export && !dispatcher_facade_export {
        anyhow::bail!(
            "Server event runtime contract drift: {} must export module_event_dispatcher directly or through the reviewed base-module facade",
            services_mod_path.display()
        );
    }
    if app_runtime.contains("spawn_index_dispatcher")
        || app_runtime.contains("spawn_search_dispatcher")
    {
        anyhow::bail!(
            "Server event runtime contract drift: {} still references legacy index/search dispatchers",
            app_runtime_path.display()
        );
    }
    if !app_runtime.contains("spawn_module_event_dispatcher") {
        anyhow::bail!(
            "Server event runtime contract drift: {} must bootstrap spawn_module_event_dispatcher",
            app_runtime_path.display()
        );
    }
    if app_runtime.contains("WorkflowTriggerHandler") {
        anyhow::bail!(
            "Server event runtime contract drift: {} must not wire WorkflowTriggerHandler directly",
            app_runtime_path.display()
        );
    }
    if !app_runtime.contains("WorkflowCronScheduler") {
        anyhow::bail!(
            "Server event runtime contract drift: {} must keep WorkflowCronScheduler as separate background runtime",
            app_runtime_path.display()
        );
    }

    Ok(())
}
