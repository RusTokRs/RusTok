use super::{
    auth_token_argument, auto_approve_argument, build_live_owner_transfer_registry_request,
    build_live_publish_registry_request, build_live_validation_stage_registry_request,
    build_live_yank_registry_request, build_module_test_plan,
    build_owner_transfer_registry_request, build_publish_registry_request,
    build_validation_stage_registry_request, build_yank_registry_request, detail_argument,
    extract_runtime_module_dependencies, load_manifest_from, module_hold_command,
    module_owner_transfer_command, module_publish_command, module_request_changes_command,
    module_resume_command, module_runner_command, module_stage_command, module_stage_run_command,
    module_test_command, module_yank_command, normalize_module_ui_classification,
    positive_u64_argument, publish_status_action_available, reason_argument, reason_code_argument,
    registry_endpoint_uses_loopback, registry_url_argument, resolve_workspace_inherited_string,
    runner_token_argument, supported_remote_runner_stages,
    validate_central_module_registry_inventory_contract, validate_default_enabled_server_contract,
    validate_host_ui_inventory_contract, validate_module_admin_surface_contract,
    validate_module_docs_navigation_contract, validate_module_entry_type_contract,
    validate_module_event_ingress_contract, validate_module_event_listener_contract,
    validate_module_host_ui_contract, validate_module_index_search_boundary_contract,
    validate_module_kind_contract, validate_module_local_docs_file,
    validate_module_permission_contract, validate_module_publish_contract,
    validate_module_registry_docs_contract, validate_module_runtime_metadata_contract,
    validate_module_search_operator_surface_contract, validate_module_semantics_contract,
    validate_module_server_http_surface_contract, validate_module_server_registry_contract,
    validate_module_transport_surface_contract, validate_module_ui_classification_contract,
    validate_module_ui_metadata_contract, validate_module_ui_surface_contract,
    validate_server_event_runtime_contract, workspace_root, Manifest, ModuleMarketplacePreview,
    ModuleOwnerTransferDryRunPreview, ModulePackageManifest, ModulePackageMetadata,
    ModulePublishDryRunPreview, ModuleSpec, ModuleUiPackagePreview, ModuleUiPackagesPreview,
    ModuleValidationStageDryRunPreview, ModuleYankDryRunPreview,
    RegistryGovernanceActionHttpResponse, RegistryPublishStatusHttpResponse, Settings,
    MODULE_AUTH_TOKEN_ENV, REGISTRY_MUTATION_SCHEMA_VERSION, REGISTRY_YANK_REASON_CODES,
    REMOTE_RUNNER_TOKEN_ENV,
};
use std::{
    collections::HashMap,
    env,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard, OnceLock},
};

struct WorkspaceRootGuard {
    previous_dir: PathBuf,
    _lock: MutexGuard<'static, ()>,
}

impl WorkspaceRootGuard {
    fn enter() -> Self {
        static WORKSPACE_CWD_GUARD: OnceLock<Mutex<()>> = OnceLock::new();

        let lock = WORKSPACE_CWD_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("workspace cwd guard should lock");
        let previous_dir = env::current_dir().expect("current dir should resolve");
        env::set_current_dir(workspace_root()).expect("workspace root should be accessible");

        Self {
            previous_dir,
            _lock: lock,
        }
    }
}

impl Drop for WorkspaceRootGuard {
    fn drop(&mut self) {
        env::set_current_dir(&self.previous_dir)
            .expect("current dir should restore after xtask test");
    }
}

struct EnvVarGuard {
    key: &'static str,
    previous_value: Option<String>,
    _lock: MutexGuard<'static, ()>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: Option<&str>) -> Self {
        static ENV_VAR_GUARD: OnceLock<Mutex<()>> = OnceLock::new();

        let lock = ENV_VAR_GUARD
            .get_or_init(|| Mutex::new(()))
            .lock()
            .expect("env var guard should lock");
        let previous_value = env::var(key).ok();
        match value {
            Some(value) => env::set_var(key, value),
            None => env::remove_var(key),
        }

        Self {
            key,
            previous_value,
            _lock: lock,
        }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.previous_value {
            Some(value) => env::set_var(self.key, value),
            None => env::remove_var(self.key),
        }
    }
}

mod commands;
mod contracts;
