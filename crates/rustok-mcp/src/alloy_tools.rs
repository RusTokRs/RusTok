#![allow(clippy::items_after_test_module)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;

pub use crate::alloy_scaffold::{
    ApplyModuleScaffoldRequest, ApplyModuleScaffoldResponse, ModuleScaffoldDraftStatus,
    ReviewModuleScaffoldRequest, ReviewModuleScaffoldResponse, ScaffoldModulePreview,
    ScaffoldModuleRequest, StageModuleScaffoldResponse, StagedModuleScaffold,
};
use alloy::model::{Script, ScriptStatus, ScriptTrigger};
use alloy::runner::ExecutionOutcome;
use alloy::storage::{ScriptQuery, ScriptRegistry};
use alloy::utils::{dynamic_to_json, json_to_dynamic};
use alloy::{EntityProxy, ScriptEngine, ScriptOrchestrator};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::runtime::{McpScaffoldDraftRuntimeContext, SharedMcpScaffoldDraftStore};

pub const TOOL_ALLOY_LIST_SCRIPTS: &str = "alloy_list_scripts";
pub const TOOL_ALLOY_GET_SCRIPT: &str = "alloy_get_script";
pub const TOOL_ALLOY_CREATE_SCRIPT: &str = "alloy_create_script";
pub const TOOL_ALLOY_UPDATE_SCRIPT: &str = "alloy_update_script";
pub const TOOL_ALLOY_DELETE_SCRIPT: &str = "alloy_delete_script";
pub const TOOL_ALLOY_VALIDATE_SCRIPT: &str = "alloy_validate_script";
pub const TOOL_ALLOY_RUN_SCRIPT: &str = "alloy_run_script";
pub const TOOL_ALLOY_SCAFFOLD_MODULE: &str = "alloy_scaffold_module";
pub const TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD: &str = "alloy_review_module_scaffold";
pub const TOOL_ALLOY_APPLY_MODULE_SCAFFOLD: &str = "alloy_apply_module_scaffold";
pub const TOOL_ALLOY_LIST_ENTITY_TYPES: &str = "alloy_list_entity_types";
pub const TOOL_ALLOY_SCRIPT_HELPERS: &str = "alloy_script_helpers";

pub const ALL_ALLOY_TOOLS: &[&str] = &[
    TOOL_ALLOY_LIST_SCRIPTS,
    TOOL_ALLOY_GET_SCRIPT,
    TOOL_ALLOY_CREATE_SCRIPT,
    TOOL_ALLOY_UPDATE_SCRIPT,
    TOOL_ALLOY_DELETE_SCRIPT,
    TOOL_ALLOY_VALIDATE_SCRIPT,
    TOOL_ALLOY_RUN_SCRIPT,
    TOOL_ALLOY_SCAFFOLD_MODULE,
    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD,
    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
    TOOL_ALLOY_LIST_ENTITY_TYPES,
    TOOL_ALLOY_SCRIPT_HELPERS,
];

pub struct AlloyMcpState<R: ScriptRegistry + 'static> {
    pub registry: Arc<R>,
    pub engine: Arc<ScriptEngine>,
    pub orchestrator: Arc<ScriptOrchestrator<R>>,
    pub staged_scaffolds: Arc<Mutex<HashMap<uuid::Uuid, StagedModuleScaffold>>>,
    pub draft_store: Option<SharedMcpScaffoldDraftStore>,
}

impl<R: ScriptRegistry + 'static> Clone for AlloyMcpState<R> {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            engine: Arc::clone(&self.engine),
            orchestrator: Arc::clone(&self.orchestrator),
            staged_scaffolds: Arc::clone(&self.staged_scaffolds),
            draft_store: self.draft_store.as_ref().map(Arc::clone),
        }
    }
}

impl<R: ScriptRegistry + 'static> AlloyMcpState<R> {
    pub fn new(
        registry: Arc<R>,
        engine: Arc<ScriptEngine>,
        orchestrator: Arc<ScriptOrchestrator<R>>,
    ) -> Self {
        Self {
            registry,
            engine,
            orchestrator,
            staged_scaffolds: Arc::new(Mutex::new(HashMap::new())),
            draft_store: None,
        }
    }

    pub fn with_shared_draft_store(mut self, draft_store: SharedMcpScaffoldDraftStore) -> Self {
        self.draft_store = Some(draft_store);
        self
    }

    pub fn with_draft_store<T>(mut self, draft_store: Arc<T>) -> Self
    where
        T: crate::runtime::McpScaffoldDraftStore + 'static,
    {
        self.draft_store = Some(draft_store);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct AlloyScriptInfo {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub code: String,
    pub trigger_type: String,
    pub status: String,
    pub version: u32,
    pub error_count: u32,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Script> for AlloyScriptInfo {
    fn from(s: Script) -> Self {
        let trigger_type = match &s.trigger {
            ScriptTrigger::Event { entity_type, event } => {
                format!("event:{}:{}", entity_type, event.as_str())
            }
            ScriptTrigger::Cron { expression } => format!("cron:{}", expression),
            ScriptTrigger::Manual => "manual".to_string(),
            ScriptTrigger::Api { path, method } => format!("api:{}:{}", method.as_str(), path),
        };
        Self {
            id: s.id.to_string(),
            name: s.name,
            description: s.description,
            code: s.code,
            trigger_type,
            status: s.status.as_str().to_string(),
            version: s.version,
            error_count: s.error_count,
            created_at: s.created_at.to_rfc3339(),
            updated_at: s.updated_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct ListScriptsRequest {
    /// Filter by status: draft, active, paused, disabled, archived. Omit to list all.
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListScriptsResponse {
    pub scripts: Vec<AlloyScriptInfo>,
    pub total: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetScriptRequest {
    /// Script name (preferred)
    pub name: Option<String>,
    /// Script UUID
    pub id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateScriptRequest {
    pub name: String,
    pub code: String,
    pub description: Option<String>,
    /// JSON-encoded trigger object. Examples:
    /// `{"type":"manual"}`,
    /// `{"type":"cron","expression":"0 * * * * *"}`,
    /// `{"type":"event","entity_type":"order","event":"before_create"}`,
    /// `{"type":"api","path":"/my-endpoint","method":"POST"}`
    pub trigger: serde_json::Value,
    /// Initial status: "draft" (default) or "active"
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UpdateScriptRequest {
    pub id: String,
    pub code: Option<String>,
    pub description: Option<String>,
    /// New status: draft, active, paused, disabled, archived
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct DeleteScriptRequest {
    pub id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateScriptRequest {
    pub code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ValidateScriptResponse {
    pub valid: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunScriptRequest {
    pub name: String,
    /// Optional JSON object with script parameters
    pub params: Option<serde_json::Value>,
    /// Optional entity context for scripts working with entities
    pub entity: Option<RunScriptEntityInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunScriptEntityInput {
    pub id: String,
    pub entity_type: String,
    pub data: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunScriptResponse {
    pub execution_id: String,
    pub success: bool,
    pub duration_ms: i64,
    pub error: Option<String>,
    pub return_value: Option<serde_json::Value>,
    pub changes: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScriptHelpersResponse {
    pub helpers: Vec<ScriptHelperInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ScriptHelperInfo {
    pub name: String,
    pub signature: String,
    pub description: String,
    pub available_in: Vec<String>,
}

pub async fn alloy_scaffold_module<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    context: Option<McpScaffoldDraftRuntimeContext>,
    request: ScaffoldModuleRequest,
) -> Result<StageModuleScaffoldResponse, String> {
    if let Some(draft_store) = &state.draft_store {
        let context = require_draft_runtime_context(context)?;
        return draft_store
            .stage_scaffold_draft(&context, request)
            .await
            .map_err(|error| error.to_string());
    }

    let preview = crate::alloy_scaffold::generate_module_scaffold(&request)?;
    let draft_id = uuid::Uuid::new_v4();
    let draft = StagedModuleScaffold {
        draft_id: draft_id.to_string(),
        request,
        preview: preview.clone(),
        status: ModuleScaffoldDraftStatus::Staged,
    };

    state
        .staged_scaffolds
        .lock()
        .map_err(|_| "Failed to lock staged scaffold store".to_string())?
        .insert(draft_id, draft);

    Ok(StageModuleScaffoldResponse {
        draft_id: draft_id.to_string(),
        preview,
        status: ModuleScaffoldDraftStatus::Staged,
        review_required: true,
        apply_tool: TOOL_ALLOY_APPLY_MODULE_SCAFFOLD.to_string(),
        next_steps: vec![
            format!(
                "Review the staged draft with {} before applying it.",
                TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD
            ),
            format!(
                "Apply the reviewed draft with {} and confirm=true.",
                TOOL_ALLOY_APPLY_MODULE_SCAFFOLD
            ),
        ],
    })
}

pub async fn alloy_review_module_scaffold<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    context: Option<McpScaffoldDraftRuntimeContext>,
    request: ReviewModuleScaffoldRequest,
) -> Result<ReviewModuleScaffoldResponse, String> {
    if let Some(draft_store) = &state.draft_store {
        let context = require_draft_runtime_context(context)?;
        return draft_store
            .review_scaffold_draft(&context, request)
            .await
            .map_err(|error| error.to_string());
    }

    let draft_id = request
        .draft_id
        .parse::<uuid::Uuid>()
        .map_err(|e| e.to_string())?;
    let draft = state
        .staged_scaffolds
        .lock()
        .map_err(|_| "Failed to lock staged scaffold store".to_string())?
        .get(&draft_id)
        .cloned()
        .ok_or_else(|| format!("Unknown scaffold draft: {}", request.draft_id))?;

    Ok(ReviewModuleScaffoldResponse { draft })
}

pub async fn alloy_apply_module_scaffold<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    context: Option<McpScaffoldDraftRuntimeContext>,
    request: ApplyModuleScaffoldRequest,
) -> Result<ApplyModuleScaffoldResponse, String> {
    if let Some(draft_store) = &state.draft_store {
        let context = require_draft_runtime_context(context)?;
        return draft_store
            .apply_scaffold_draft(&context, request)
            .await
            .map_err(|error| error.to_string());
    }

    if !request.confirm {
        return Err(
            "Refusing to apply staged scaffold without confirm=true after review.".to_string(),
        );
    }

    let draft_id = request
        .draft_id
        .parse::<uuid::Uuid>()
        .map_err(|e| e.to_string())?;
    let mut store = state
        .staged_scaffolds
        .lock()
        .map_err(|_| "Failed to lock staged scaffold store".to_string())?;
    let draft = store
        .get_mut(&draft_id)
        .ok_or_else(|| format!("Unknown scaffold draft: {}", request.draft_id))?;

    if draft.status == ModuleScaffoldDraftStatus::Applied {
        return Err(format!(
            "Scaffold draft {} has already been applied",
            request.draft_id
        ));
    }

    let response = crate::alloy_scaffold::apply_staged_scaffold(draft, &request.workspace_root)?;
    draft.status = ModuleScaffoldDraftStatus::Applied;
    Ok(response)
}

fn require_draft_runtime_context(
    context: Option<McpScaffoldDraftRuntimeContext>,
) -> Result<McpScaffoldDraftRuntimeContext, String> {
    context.ok_or_else(|| {
        "Persisted scaffold draft store requires MCP runtime context with tenant/client binding"
            .to_string()
    })
}

pub async fn alloy_list_scripts<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    request: ListScriptsRequest,
) -> Result<ListScriptsResponse, String> {
    let query = match request.status.as_deref() {
        Some(s) => match ScriptStatus::parse(s) {
            Some(status) => ScriptQuery::ByStatus(status),
            None => return Err(format!("Invalid status: {s}")),
        },
        None => ScriptQuery::All,
    };

    let scripts = state
        .registry
        .find(query)
        .await
        .map_err(|e| e.to_string())?;
    let total = scripts.len();
    Ok(ListScriptsResponse {
        scripts: scripts.into_iter().map(AlloyScriptInfo::from).collect(),
        total,
    })
}

pub async fn alloy_get_script<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    request: GetScriptRequest,
) -> Result<AlloyScriptInfo, String> {
    if let Some(name) = request.name {
        let script = state
            .registry
            .get_by_name(&name)
            .await
            .map_err(|e| e.to_string())?;
        return Ok(script.into());
    }
    if let Some(id_str) = request.id {
        let id = id_str.parse::<uuid::Uuid>().map_err(|e| e.to_string())?;
        let script = state.registry.get(id).await.map_err(|e| e.to_string())?;
        return Ok(script.into());
    }
    Err("Either name or id must be provided".to_string())
}

pub async fn alloy_create_script<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    request: CreateScriptRequest,
) -> Result<AlloyScriptInfo, String> {
    let trigger: ScriptTrigger =
        serde_json::from_value(request.trigger).map_err(|e| e.to_string())?;

    if let ScriptTrigger::Cron { ref expression } = trigger {
        alloy::utils::validate_cron_expression(expression)?;
    }

    let mut scope = rhai::Scope::new();
    state
        .engine
        .compile(&request.name, &request.code, &mut scope)
        .map_err(|e| e.to_string())?;

    let mut script = Script::new(request.name, request.code, trigger);
    script.description = request.description;

    if let Some(status_str) = request.status {
        script.status = ScriptStatus::parse(&status_str)
            .ok_or_else(|| format!("Invalid status: {status_str}"))?;
    }

    let saved = state
        .registry
        .save(script)
        .await
        .map_err(|e| e.to_string())?;
    Ok(saved.into())
}

pub async fn alloy_update_script<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    request: UpdateScriptRequest,
) -> Result<AlloyScriptInfo, String> {
    let id = request
        .id
        .parse::<uuid::Uuid>()
        .map_err(|e| e.to_string())?;
    let mut script = state.registry.get(id).await.map_err(|e| e.to_string())?;

    if let Some(code) = request.code {
        let mut scope = rhai::Scope::new();
        state
            .engine
            .compile(&script.name, &code, &mut scope)
            .map_err(|e| e.to_string())?;
        state.engine.invalidate(&script.name);
        script.code = code;
    }

    if let Some(desc) = request.description {
        script.description = Some(desc);
    }

    if let Some(status_str) = request.status {
        script.status = ScriptStatus::parse(&status_str)
            .ok_or_else(|| format!("Invalid status: {status_str}"))?;
    }

    let saved = state
        .registry
        .save(script)
        .await
        .map_err(|e| e.to_string())?;
    Ok(saved.into())
}

pub async fn alloy_delete_script<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    request: DeleteScriptRequest,
) -> Result<bool, String> {
    let id = request
        .id
        .parse::<uuid::Uuid>()
        .map_err(|e| e.to_string())?;
    let script = state.registry.get(id).await.map_err(|e| e.to_string())?;
    state.engine.invalidate(&script.name);
    state.registry.delete(id).await.map_err(|e| e.to_string())?;
    Ok(true)
}

pub fn alloy_validate_script<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    request: ValidateScriptRequest,
) -> ValidateScriptResponse {
    let mut scope = rhai::Scope::new();
    match state
        .engine
        .compile("__mcp_validate__", &request.code, &mut scope)
    {
        Ok(_) => ValidateScriptResponse {
            valid: true,
            message: "Script compiles successfully".to_string(),
        },
        Err(e) => ValidateScriptResponse {
            valid: false,
            message: e.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy::{create_default_engine, InMemoryStorage, ScriptOrchestrator};
    use anyhow::Result as AnyhowResult;
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::runtime::McpScaffoldDraftStore;

    fn test_state() -> AlloyMcpState<InMemoryStorage> {
        let engine = Arc::new(create_default_engine());
        let storage = Arc::new(InMemoryStorage::new());
        let orchestrator = Arc::new(ScriptOrchestrator::new(engine.clone(), storage.clone()));
        AlloyMcpState::new(storage, engine, orchestrator)
    }

    fn draft_runtime_context() -> McpScaffoldDraftRuntimeContext {
        McpScaffoldDraftRuntimeContext {
            session: crate::runtime::McpSessionContext::stdio(),
            runtime_binding: None,
            access_context: None,
        }
    }

    #[tokio::test]
    async fn stage_and_review_scaffold() {
        let state = test_state();
        let staged = alloy_scaffold_module(
            &state,
            None,
            ScaffoldModuleRequest {
                slug: "newsletter".to_string(),
                name: "Newsletter".to_string(),
                description: "newsletter module".to_string(),
                dependencies: Vec::new(),
                with_graphql: true,
                with_rest: true,
                write_files: false,
            },
        )
        .await
        .expect("stage should succeed");

        assert_eq!(staged.status, ModuleScaffoldDraftStatus::Staged);

        let reviewed = alloy_review_module_scaffold(
            &state,
            None,
            ReviewModuleScaffoldRequest {
                draft_id: staged.draft_id.clone(),
            },
        )
        .await
        .expect("review should succeed");

        assert_eq!(reviewed.draft.draft_id, staged.draft_id);
        assert_eq!(reviewed.draft.status, ModuleScaffoldDraftStatus::Staged);
    }

    #[tokio::test]
    async fn apply_scaffold_requires_confirmation() {
        let state = test_state();
        let staged = alloy_scaffold_module(
            &state,
            None,
            ScaffoldModuleRequest {
                slug: "newsletter".to_string(),
                name: "Newsletter".to_string(),
                description: "newsletter module".to_string(),
                dependencies: Vec::new(),
                with_graphql: false,
                with_rest: false,
                write_files: false,
            },
        )
        .await
        .expect("stage should succeed");

        let error = alloy_apply_module_scaffold(
            &state,
            None,
            ApplyModuleScaffoldRequest {
                draft_id: staged.draft_id,
                workspace_root: "C:\\tmp".to_string(),
                confirm: false,
            },
        )
        .await
        .expect_err("confirm=false must be rejected");

        assert!(error.contains("confirm=true"));
    }

    struct FakeDraftStore {
        stage_calls: AtomicUsize,
        review_calls: AtomicUsize,
        apply_calls: AtomicUsize,
    }

    #[async_trait]
    impl McpScaffoldDraftStore for FakeDraftStore {
        async fn stage_scaffold_draft(
            &self,
            _context: &McpScaffoldDraftRuntimeContext,
            request: ScaffoldModuleRequest,
        ) -> AnyhowResult<StageModuleScaffoldResponse> {
            self.stage_calls.fetch_add(1, Ordering::SeqCst);
            Ok(StageModuleScaffoldResponse {
                draft_id: "persisted-draft".to_string(),
                preview: crate::alloy_scaffold::generate_module_scaffold(&request)
                    .map_err(anyhow::Error::msg)?,
                status: ModuleScaffoldDraftStatus::Staged,
                review_required: true,
                apply_tool: TOOL_ALLOY_APPLY_MODULE_SCAFFOLD.to_string(),
                next_steps: vec!["persisted".to_string()],
            })
        }

        async fn review_scaffold_draft(
            &self,
            _context: &McpScaffoldDraftRuntimeContext,
            request: ReviewModuleScaffoldRequest,
        ) -> AnyhowResult<ReviewModuleScaffoldResponse> {
            self.review_calls.fetch_add(1, Ordering::SeqCst);
            Ok(ReviewModuleScaffoldResponse {
                draft: StagedModuleScaffold {
                    draft_id: request.draft_id,
                    request: ScaffoldModuleRequest {
                        slug: "newsletter".to_string(),
                        name: "Newsletter".to_string(),
                        description: "persisted".to_string(),
                        dependencies: Vec::new(),
                        with_graphql: true,
                        with_rest: true,
                        write_files: false,
                    },
                    preview: ScaffoldModulePreview {
                        crate_name: "rustok-newsletter".to_string(),
                        crate_path: "crates/rustok-newsletter".to_string(),
                        files: Vec::new(),
                        next_steps: vec!["persisted".to_string()],
                    },
                    status: ModuleScaffoldDraftStatus::Staged,
                },
            })
        }

        async fn apply_scaffold_draft(
            &self,
            _context: &McpScaffoldDraftRuntimeContext,
            request: ApplyModuleScaffoldRequest,
        ) -> AnyhowResult<ApplyModuleScaffoldResponse> {
            self.apply_calls.fetch_add(1, Ordering::SeqCst);
            Ok(ApplyModuleScaffoldResponse {
                draft_id: request.draft_id,
                crate_name: "rustok-newsletter".to_string(),
                crate_path: "crates/rustok-newsletter".to_string(),
                wrote_files: true,
                status: ModuleScaffoldDraftStatus::Applied,
                next_steps: vec!["persisted".to_string()],
            })
        }
    }

    #[tokio::test]
    async fn persisted_store_replaces_in_memory_scaffold_flow() {
        let draft_store = Arc::new(FakeDraftStore {
            stage_calls: AtomicUsize::new(0),
            review_calls: AtomicUsize::new(0),
            apply_calls: AtomicUsize::new(0),
        });
        let state = test_state().with_draft_store(Arc::clone(&draft_store));

        let staged = alloy_scaffold_module(
            &state,
            Some(draft_runtime_context()),
            ScaffoldModuleRequest {
                slug: "newsletter".to_string(),
                name: "Newsletter".to_string(),
                description: "newsletter module".to_string(),
                dependencies: Vec::new(),
                with_graphql: true,
                with_rest: true,
                write_files: false,
            },
        )
        .await
        .expect("persisted stage should succeed");
        assert_eq!(staged.draft_id, "persisted-draft");

        let reviewed = alloy_review_module_scaffold(
            &state,
            Some(draft_runtime_context()),
            ReviewModuleScaffoldRequest {
                draft_id: staged.draft_id.clone(),
            },
        )
        .await
        .expect("persisted review should succeed");
        assert_eq!(reviewed.draft.draft_id, staged.draft_id);

        let applied = alloy_apply_module_scaffold(
            &state,
            Some(draft_runtime_context()),
            ApplyModuleScaffoldRequest {
                draft_id: staged.draft_id,
                workspace_root: "C:\\tmp".to_string(),
                confirm: true,
            },
        )
        .await
        .expect("persisted apply should succeed");
        assert_eq!(applied.status, ModuleScaffoldDraftStatus::Applied);

        assert_eq!(draft_store.stage_calls.load(Ordering::SeqCst), 1);
        assert_eq!(draft_store.review_calls.load(Ordering::SeqCst), 1);
        assert_eq!(draft_store.apply_calls.load(Ordering::SeqCst), 1);
        assert!(state
            .staged_scaffolds
            .lock()
            .expect("in-memory store lock")
            .is_empty());
    }
}

pub async fn alloy_run_script<R: ScriptRegistry>(
    state: &AlloyMcpState<R>,
    request: RunScriptRequest,
) -> Result<RunScriptResponse, String> {
    let params = match request.params {
        Some(serde_json::Value::Object(map)) => map
            .into_iter()
            .map(|(k, v)| (k, json_to_dynamic(v)))
            .collect(),
        _ => HashMap::new(),
    };

    let entity = request.entity.map(|e| {
        let data = match e.data {
            serde_json::Value::Object(map) => map
                .into_iter()
                .map(|(k, v)| (k, json_to_dynamic(v)))
                .collect(),
            _ => HashMap::new(),
        };
        EntityProxy::new(e.id, e.entity_type, data)
    });

    let result = state
        .orchestrator
        .run_manual_with_entity(&request.name, params, entity, None)
        .await
        .map_err(|e| e.to_string())?;

    let (success, error, return_value, changes) = match &result.outcome {
        ExecutionOutcome::Success {
            return_value,
            entity_changes,
        } => {
            let rv = return_value.clone().map(dynamic_to_json);
            let ch = serde_json::Value::Object(
                entity_changes
                    .iter()
                    .map(|(k, v)| (k.clone(), dynamic_to_json(v.clone())))
                    .collect(),
            );
            (true, None, rv, Some(ch))
        }
        ExecutionOutcome::Aborted { reason } => (false, Some(reason.clone()), None, None),
        ExecutionOutcome::Failed { error } => (false, Some(error.to_string()), None, None),
    };

    Ok(RunScriptResponse {
        execution_id: result.execution_id.to_string(),
        success,
        duration_ms: result.duration_ms(),
        error,
        return_value,
        changes,
    })
}

pub fn alloy_list_entity_types() -> Vec<String> {
    vec![
        "order".to_string(),
        "product".to_string(),
        "content".to_string(),
        "page".to_string(),
        "blog_post".to_string(),
        "forum_topic".to_string(),
        "forum_post".to_string(),
        "user".to_string(),
        "tenant".to_string(),
    ]
}

pub fn alloy_script_helpers() -> ScriptHelpersResponse {
    ScriptHelpersResponse {
        helpers: vec![
            ScriptHelperInfo {
                name: "log".to_string(),
                signature: "log(message: string)".to_string(),
                description: "Log an info message".to_string(),
                available_in: vec!["all".to_string()],
            },
            ScriptHelperInfo {
                name: "log_warn".to_string(),
                signature: "log_warn(message: string)".to_string(),
                description: "Log a warning message".to_string(),
                available_in: vec!["all".to_string()],
            },
            ScriptHelperInfo {
                name: "log_error".to_string(),
                signature: "log_error(message: string)".to_string(),
                description: "Log an error message".to_string(),
                available_in: vec!["all".to_string()],
            },
            ScriptHelperInfo {
                name: "abort".to_string(),
                signature: "abort(reason: string)".to_string(),
                description: "Abort execution and reject the operation with a reason".to_string(),
                available_in: vec!["before".to_string()],
            },
            ScriptHelperInfo {
                name: "now".to_string(),
                signature: "now() -> string".to_string(),
                description: "Return current UTC timestamp as RFC3339 string".to_string(),
                available_in: vec!["all".to_string()],
            },
            ScriptHelperInfo {
                name: "now_unix".to_string(),
                signature: "now_unix() -> i64".to_string(),
                description: "Return current Unix timestamp in seconds".to_string(),
                available_in: vec!["all".to_string()],
            },
            ScriptHelperInfo {
                name: "validate_email".to_string(),
                signature: "validate_email(email: string) -> bool".to_string(),
                description: "Validate an email address format".to_string(),
                available_in: vec![
                    "before".to_string(),
                    "manual".to_string(),
                    "scheduled".to_string(),
                ],
            },
            ScriptHelperInfo {
                name: "validate_required".to_string(),
                signature: "validate_required(value: string) -> bool".to_string(),
                description: "Check that a string is non-empty after trimming".to_string(),
                available_in: vec![
                    "before".to_string(),
                    "manual".to_string(),
                    "scheduled".to_string(),
                ],
            },
            ScriptHelperInfo {
                name: "validate_min_length".to_string(),
                signature: "validate_min_length(value: string, min: i64) -> bool".to_string(),
                description: "Check that a string has at least min characters".to_string(),
                available_in: vec![
                    "before".to_string(),
                    "manual".to_string(),
                    "scheduled".to_string(),
                ],
            },
            ScriptHelperInfo {
                name: "validate_max_length".to_string(),
                signature: "validate_max_length(value: string, max: i64) -> bool".to_string(),
                description: "Check that a string has at most max characters".to_string(),
                available_in: vec![
                    "before".to_string(),
                    "manual".to_string(),
                    "scheduled".to_string(),
                ],
            },
            ScriptHelperInfo {
                name: "validate_range".to_string(),
                signature: "validate_range(value: i64, min: i64, max: i64) -> bool".to_string(),
                description: "Check that a number is within [min, max]".to_string(),
                available_in: vec![
                    "before".to_string(),
                    "manual".to_string(),
                    "scheduled".to_string(),
                ],
            },
            ScriptHelperInfo {
                name: "format_money".to_string(),
                signature: "format_money(amount: i64) -> string".to_string(),
                description: "Format an integer amount with thousands separators".to_string(),
                available_in: vec!["all".to_string()],
            },
            ScriptHelperInfo {
                name: "is_empty".to_string(),
                signature: "is_empty(value: Dynamic) -> bool".to_string(),
                description: "Check if a value is null, empty string, or empty array".to_string(),
                available_in: vec!["all".to_string()],
            },
            ScriptHelperInfo {
                name: "coalesce".to_string(),
                signature: "coalesce(value: Dynamic, default: Dynamic) -> Dynamic".to_string(),
                description: "Return value if non-null, otherwise return default".to_string(),
                available_in: vec!["all".to_string()],
            },
        ],
    }
}
