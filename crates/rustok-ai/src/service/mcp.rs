use once_cell::sync::Lazy;
use schemars::schema_for;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

use rustok_mcp::alloy_tools::{
    self, AlloyMcpState, ApplyModuleScaffoldRequest, CreateScriptRequest, DeleteScriptRequest,
    GetScriptRequest, ListScriptsRequest, ReviewModuleScaffoldRequest, RunScriptRequest,
    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD, TOOL_ALLOY_CREATE_SCRIPT, TOOL_ALLOY_DELETE_SCRIPT,
    TOOL_ALLOY_GET_SCRIPT, TOOL_ALLOY_LIST_ENTITY_TYPES, TOOL_ALLOY_LIST_SCRIPTS,
    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD, TOOL_ALLOY_RUN_SCRIPT, TOOL_ALLOY_SCAFFOLD_MODULE,
    TOOL_ALLOY_SCRIPT_HELPERS, TOOL_ALLOY_UPDATE_SCRIPT, TOOL_ALLOY_VALIDATE_SCRIPT,
    UpdateScriptRequest, ValidateScriptRequest,
};
use rustok_mcp::tools::{
    self, McpHealthResponse, McpState, McpToolResponse, ModuleLookupRequest, ModuleQueryRequest,
    TOOL_BLOG_MODULE, TOOL_CONTENT_MODULE, TOOL_FORUM_MODULE, TOOL_LIST_MODULES, TOOL_MCP_HEALTH,
    TOOL_MCP_WHOAMI, TOOL_MODULE_DETAILS, TOOL_MODULE_EXISTS, TOOL_PAGES_MODULE,
    TOOL_QUERY_MODULES,
};
use rustok_mcp::{
    McpAccessContext, McpAccessPolicy, McpActorType, McpIdentity, StagedModuleScaffold,
    default_tool_requirement,
};

use crate::mcp::{McpClientAdapter, ToolExecutionResult};
use crate::model::ToolDefinition;
use crate::{AiError, AiResult};

use super::helpers::{json_err, parse_uuid_str};
use super::types::{AiHostRuntime, AiOperatorContext};

static STAGED_SCAFFOLDS: Lazy<Arc<Mutex<HashMap<Uuid, StagedModuleScaffold>>>> =
    Lazy::new(|| Arc::new(Mutex::new(HashMap::new())));

pub struct InProcessMcpAdapter {
    pub state: McpState,
    pub access_context: McpAccessContext,
    pub alloy: Option<AlloyMcpState<alloy::SeaOrmStorage>>,
}

impl InProcessMcpAdapter {
    pub fn new(runtime: &AiHostRuntime, access_context: McpAccessContext) -> AiResult<Self> {
        let registry = runtime.module_registry();
        let tenant_id = parse_uuid_str(
            access_context
                .identity
                .as_ref()
                .and_then(|identity| identity.tenant_id.as_deref()),
        )?;
        let alloy = if let Some(scoped) = runtime.scoped_alloy_runtime(tenant_id) {
            let mut state = AlloyMcpState::new(scoped.storage, scoped.engine, scoped.orchestrator);
            state.staged_scaffolds = Arc::clone(&STAGED_SCAFFOLDS);
            Some(state)
        } else {
            None
        };
        Ok(Self {
            state: McpState { registry },
            access_context,
            alloy,
        })
    }

    async fn call_alloy_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> AiResult<ToolExecutionResult> {
        let Some(state) = &self.alloy else {
            return Err(AiError::Mcp(format!("unknown tool: {tool_name}")));
        };
        let content = match tool_name {
            TOOL_ALLOY_LIST_SCRIPTS => serde_json::to_value(
                alloy_tools::alloy_list_scripts(
                    state,
                    serde_json::from_value::<ListScriptsRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_GET_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_get_script(
                    state,
                    serde_json::from_value::<GetScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_CREATE_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_create_script(
                    state,
                    serde_json::from_value::<CreateScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_UPDATE_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_update_script(
                    state,
                    serde_json::from_value::<UpdateScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_DELETE_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_delete_script(
                    state,
                    serde_json::from_value::<DeleteScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_VALIDATE_SCRIPT => serde_json::to_value(alloy_tools::alloy_validate_script(
                state,
                serde_json::from_value::<ValidateScriptRequest>(input).map_err(json_err)?,
            ))
            .map_err(json_err)?,
            TOOL_ALLOY_RUN_SCRIPT => serde_json::to_value(
                alloy_tools::alloy_run_script(
                    state,
                    serde_json::from_value::<RunScriptRequest>(input).map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_SCAFFOLD_MODULE => serde_json::to_value(
                alloy_tools::alloy_scaffold_module(
                    state,
                    None,
                    serde_json::from_value::<rustok_mcp::ScaffoldModuleRequest>(input)
                        .map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD => serde_json::to_value(
                alloy_tools::alloy_review_module_scaffold(
                    state,
                    None,
                    serde_json::from_value::<ReviewModuleScaffoldRequest>(input)
                        .map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_APPLY_MODULE_SCAFFOLD => serde_json::to_value(
                alloy_tools::alloy_apply_module_scaffold(
                    state,
                    None,
                    serde_json::from_value::<ApplyModuleScaffoldRequest>(input)
                        .map_err(json_err)?,
                )
                .await
                .map_err(AiError::Mcp)?,
            )
            .map_err(json_err)?,
            TOOL_ALLOY_LIST_ENTITY_TYPES => {
                serde_json::to_value(alloy_tools::alloy_list_entity_types()).map_err(json_err)?
            }
            TOOL_ALLOY_SCRIPT_HELPERS => {
                serde_json::to_value(alloy_tools::alloy_script_helpers()).map_err(json_err)?
            }
            _ => return Err(AiError::Mcp(format!("unknown tool: {tool_name}"))),
        };

        Ok(ToolExecutionResult {
            content: serde_json::to_string(&content).map_err(json_err)?,
            raw_payload: content,
        })
    }
}

#[async_trait::async_trait]
impl McpClientAdapter for InProcessMcpAdapter {
    async fn list_tools(&self) -> AiResult<Vec<ToolDefinition>> {
        let mut tools = vec![
            tool_def(
                TOOL_LIST_MODULES,
                "List all registered RusToK modules with their metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_QUERY_MODULES,
                "List modules with filters and pagination",
                schema_for!(ModuleQueryRequest),
            ),
            tool_def(
                TOOL_MODULE_EXISTS,
                "Check if a module exists by its slug",
                schema_for!(ModuleLookupRequest),
            ),
            tool_def(
                TOOL_MODULE_DETAILS,
                "Fetch module metadata by slug",
                schema_for!(ModuleLookupRequest),
            ),
            tool_def(
                TOOL_CONTENT_MODULE,
                "Fetch content module metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_BLOG_MODULE,
                "Fetch blog module metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_FORUM_MODULE,
                "Fetch forum module metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_PAGES_MODULE,
                "Fetch pages module metadata",
                schema_for!(()),
            ),
            tool_def(
                TOOL_MCP_HEALTH,
                "MCP readiness and configuration status",
                schema_for!(()),
            ),
            tool_def(
                TOOL_MCP_WHOAMI,
                "Inspect the current MCP identity, permissions, scopes, and tool policy",
                schema_for!(()),
            ),
        ];

        if self.alloy.is_some() {
            tools.extend([
                tool_def(
                    TOOL_ALLOY_LIST_SCRIPTS,
                    "List Alloy scripts with optional status filter",
                    schema_for!(ListScriptsRequest),
                ),
                tool_def(
                    TOOL_ALLOY_GET_SCRIPT,
                    "Get a single Alloy script by name or UUID",
                    schema_for!(GetScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_CREATE_SCRIPT,
                    "Create a new Alloy Rhai script",
                    schema_for!(CreateScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_UPDATE_SCRIPT,
                    "Update an existing Alloy script (code, description, status)",
                    schema_for!(UpdateScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_DELETE_SCRIPT,
                    "Delete an Alloy script by UUID",
                    schema_for!(DeleteScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_VALIDATE_SCRIPT,
                    "Validate Rhai script syntax without executing",
                    schema_for!(ValidateScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_RUN_SCRIPT,
                    "Execute an Alloy script manually with optional params and entity context",
                    schema_for!(RunScriptRequest),
                ),
                tool_def(
                    TOOL_ALLOY_SCAFFOLD_MODULE,
                    "Stage a reviewed draft RusToK module crate scaffold without writing it into the workspace yet",
                    schema_for!(rustok_mcp::ScaffoldModuleRequest),
                ),
                tool_def(
                    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD,
                    "Fetch a staged Alloy module scaffold draft for review before apply",
                    schema_for!(ReviewModuleScaffoldRequest),
                ),
                tool_def(
                    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
                    "Apply a reviewed Alloy module scaffold draft into the workspace with explicit confirmation",
                    schema_for!(ApplyModuleScaffoldRequest),
                ),
                tool_def(
                    TOOL_ALLOY_LIST_ENTITY_TYPES,
                    "List all known entity types in the platform",
                    schema_for!(()),
                ),
                tool_def(
                    TOOL_ALLOY_SCRIPT_HELPERS,
                    "List available Rhai helper functions with signatures and descriptions",
                    schema_for!(()),
                ),
            ]);
        }

        Ok(tools
            .into_iter()
            .filter(|tool| {
                self.access_context
                    .authorize_tool(&default_tool_requirement(&tool.name))
                    .allowed
            })
            .collect())
    }

    async fn call_tool(
        &self,
        tool_name: &str,
        input: serde_json::Value,
    ) -> AiResult<ToolExecutionResult> {
        let decision = self
            .access_context
            .authorize_tool(&default_tool_requirement(tool_name));
        if !decision.allowed {
            return Err(AiError::Mcp(
                decision
                    .message
                    .unwrap_or_else(|| "tool access denied".to_string()),
            ));
        }

        match tool_name {
            TOOL_LIST_MODULES => serialize_result(McpToolResponse::success(
                tools::list_modules(&self.state).await,
            )),
            TOOL_QUERY_MODULES => serialize_result(McpToolResponse::success(
                tools::list_modules_filtered(
                    &self.state,
                    serde_json::from_value::<ModuleQueryRequest>(input).map_err(json_err)?,
                )
                .await,
            )),
            TOOL_MODULE_EXISTS => serialize_result(McpToolResponse::success(
                tools::module_exists(
                    &self.state,
                    serde_json::from_value::<ModuleLookupRequest>(input).map_err(json_err)?,
                )
                .await,
            )),
            TOOL_MODULE_DETAILS => serialize_result(McpToolResponse::success(
                tools::module_details(
                    &self.state,
                    serde_json::from_value::<ModuleLookupRequest>(input).map_err(json_err)?,
                )
                .await,
            )),
            TOOL_CONTENT_MODULE => serialize_result(McpToolResponse::success(
                tools::module_details_by_slug(&self.state, rustok_mcp::MODULE_CONTENT),
            )),
            TOOL_BLOG_MODULE => serialize_result(McpToolResponse::success(
                tools::module_details_by_slug(&self.state, rustok_mcp::MODULE_BLOG),
            )),
            TOOL_FORUM_MODULE => serialize_result(McpToolResponse::success(
                tools::module_details_by_slug(&self.state, rustok_mcp::MODULE_FORUM),
            )),
            TOOL_PAGES_MODULE => serialize_result(McpToolResponse::success(
                tools::module_details_by_slug(&self.state, rustok_mcp::MODULE_PAGES),
            )),
            TOOL_MCP_HEALTH => serialize_result(McpToolResponse::success(McpHealthResponse {
                status: "ok".to_string(),
                protocol_version: "in_process".to_string(),
                tool_count: self.list_tools().await?.len(),
                enabled_tools: None,
                access_mode: "direct".to_string(),
                identity: self.access_context.identity.clone(),
            })),
            TOOL_MCP_WHOAMI => {
                serialize_result(McpToolResponse::success(self.access_context.whoami()))
            }
            _ => self.call_alloy_tool(tool_name, input).await,
        }
    }
}

pub fn tool_def(name: &str, description: &str, schema: schemars::Schema) -> ToolDefinition {
    let input_schema = serde_json::to_value(schema)
        .ok()
        .and_then(|value| value.as_object().cloned())
        .map(serde_json::Value::Object)
        .unwrap_or_else(|| json!({}));
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        input_schema,
        sensitive: false,
    }
}

pub fn serialize_result<T: Serialize>(payload: T) -> AiResult<ToolExecutionResult> {
    let raw_payload = serde_json::to_value(payload).map_err(json_err)?;
    Ok(ToolExecutionResult {
        content: serde_json::to_string(&raw_payload).map_err(json_err)?,
        raw_payload,
    })
}

pub fn access_context_for_operator(operator: &AiOperatorContext) -> McpAccessContext {
    McpAccessContext {
        identity: Some(McpIdentity {
            actor_id: operator.user_id.to_string(),
            actor_type: McpActorType::HumanUser,
            tenant_id: Some(operator.tenant_id.to_string()),
            delegated_user_id: None,
            display_name: Some("RusToK AI Operator".to_string()),
            scopes: Vec::new(),
        }),
        granted_permissions: operator
            .permissions
            .iter()
            .map(ToString::to_string)
            .collect(),
        policy: McpAccessPolicy {
            allowed_tools: None,
            denied_tools: Vec::new(),
        },
    }
}
