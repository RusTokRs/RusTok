use std::collections::HashSet;
use std::sync::Arc;

use anyhow::Result;
use rmcp::{
    ServerHandler, ServiceExt,
    model::{CallToolRequestParams, CallToolResult, Implementation, ListToolsResult, ServerInfo},
    service::{RequestContext, RoleServer},
    transport::stdio,
};
use rustok_core::registry::ModuleRegistry;

use crate::access::{McpAccessContext, McpWhoAmIResponse, default_tool_requirement};
use crate::alloy_tools::{
    ALL_ALLOY_TOOLS, AlloyMcpState, ApplyModuleScaffoldRequest, CreateScriptRequest,
    DeleteScriptRequest, GetScriptRequest, ListScriptsRequest, ReviewModuleScaffoldRequest,
    RunScriptRequest, ScaffoldModuleRequest, TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
    TOOL_ALLOY_CREATE_SCRIPT, TOOL_ALLOY_DELETE_SCRIPT, TOOL_ALLOY_GET_SCRIPT,
    TOOL_ALLOY_LIST_ENTITY_TYPES, TOOL_ALLOY_LIST_SCRIPTS, TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD,
    TOOL_ALLOY_RUN_SCRIPT, TOOL_ALLOY_SCAFFOLD_MODULE, TOOL_ALLOY_SCRIPT_HELPERS,
    TOOL_ALLOY_UPDATE_SCRIPT, TOOL_ALLOY_VALIDATE_SCRIPT, UpdateScriptRequest,
    ValidateScriptRequest, alloy_apply_module_scaffold, alloy_create_script, alloy_delete_script,
    alloy_get_script, alloy_list_entity_types, alloy_list_scripts, alloy_review_module_scaffold,
    alloy_run_script, alloy_scaffold_module, alloy_script_helpers, alloy_update_script,
    alloy_validate_script,
};
use crate::runtime::{
    McpRuntimeBinding, McpScaffoldDraftRuntimeContext, McpSessionContext, McpToolCallAuditEvent,
    SharedMcpAccessResolver, SharedMcpAuditSink,
};
use crate::tools::{
    MODULE_BLOG, MODULE_CONTENT, MODULE_FORUM, MODULE_PAGES, McpHealthResponse, McpState,
    McpToolResponse, ModuleDetailsResponse, ModuleListResponse, ModuleLookupRequest,
    ModuleLookupResponse, ModuleQueryRequest, TOOL_BLOG_MODULE, TOOL_CONTENT_MODULE,
    TOOL_FORUM_MODULE, TOOL_LIST_MODULES, TOOL_MCP_HEALTH, TOOL_MCP_WHOAMI, TOOL_MODULE_DETAILS,
    TOOL_MODULE_EXISTS, TOOL_PAGES_MODULE, TOOL_QUERY_MODULES, list_modules, list_modules_filtered,
    module_details, module_details_by_slug, module_exists,
};
use alloy::storage::ScriptRegistry;

/// Configuration for the MCP server
pub struct McpServerConfig {
    pub registry: ModuleRegistry,
    pub enabled_tools: Option<HashSet<String>>,
    pub access_context: Option<McpAccessContext>,
    pub session_context: McpSessionContext,
    pub access_resolver: Option<SharedMcpAccessResolver>,
    pub audit_sink: Option<SharedMcpAuditSink>,
}

impl McpServerConfig {
    pub fn new(registry: ModuleRegistry) -> Self {
        Self {
            registry,
            enabled_tools: None,
            access_context: None,
            session_context: McpSessionContext::default(),
            access_resolver: None,
            audit_sink: None,
        }
    }

    pub fn with_enabled_tools<I, S>(registry: ModuleRegistry, enabled_tools: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            registry,
            enabled_tools: Some(enabled_tools.into_iter().map(Into::into).collect()),
            access_context: None,
            session_context: McpSessionContext::default(),
            access_resolver: None,
            audit_sink: None,
        }
    }

    pub fn with_access_context(mut self, access_context: McpAccessContext) -> Self {
        self.access_context = Some(access_context);
        self
    }

    pub fn with_session_context(mut self, session_context: McpSessionContext) -> Self {
        self.session_context = session_context;
        self
    }

    pub fn with_access_resolver<T>(mut self, access_resolver: Arc<T>) -> Self
    where
        T: crate::runtime::McpAccessResolver + 'static,
    {
        self.access_resolver = Some(access_resolver);
        self
    }

    pub fn with_audit_sink<T>(mut self, audit_sink: Arc<T>) -> Self
    where
        T: crate::runtime::McpAuditSink + 'static,
    {
        self.audit_sink = Some(audit_sink);
        self
    }
}

/// MCP Server handler for RusToK modules
pub struct RusToKMcpServer<R: ScriptRegistry + 'static = alloy::InMemoryStorage> {
    state: Arc<McpState>,
    alloy: Option<Arc<AlloyMcpState<R>>>,
    enabled_tools: Option<Arc<HashSet<String>>>,
    access_context: Option<Arc<McpAccessContext>>,
    runtime_binding: Option<Arc<McpRuntimeBinding>>,
    session_context: Arc<McpSessionContext>,
    audit_sink: Option<SharedMcpAuditSink>,
}

impl<R: ScriptRegistry + 'static> Clone for RusToKMcpServer<R> {
    fn clone(&self) -> Self {
        Self {
            state: Arc::clone(&self.state),
            alloy: self.alloy.as_ref().map(Arc::clone),
            enabled_tools: self.enabled_tools.as_ref().map(Arc::clone),
            access_context: self.access_context.as_ref().map(Arc::clone),
            runtime_binding: self.runtime_binding.as_ref().map(Arc::clone),
            session_context: Arc::clone(&self.session_context),
            audit_sink: self.audit_sink.as_ref().map(Arc::clone),
        }
    }
}

impl RusToKMcpServer<alloy::InMemoryStorage> {
    pub fn new(registry: ModuleRegistry) -> Self {
        Self {
            state: Arc::new(McpState { registry }),
            alloy: None,
            enabled_tools: None,
            access_context: None,
            runtime_binding: None,
            session_context: Arc::new(McpSessionContext::default()),
            audit_sink: None,
        }
    }

    pub fn with_enabled_tools(registry: ModuleRegistry, enabled_tools: HashSet<String>) -> Self {
        Self {
            state: Arc::new(McpState { registry }),
            alloy: None,
            enabled_tools: Some(Arc::new(enabled_tools)),
            access_context: None,
            runtime_binding: None,
            session_context: Arc::new(McpSessionContext::default()),
            audit_sink: None,
        }
    }
}

impl<R: ScriptRegistry + 'static> RusToKMcpServer<R> {
    pub fn with_alloy(registry: ModuleRegistry, alloy: AlloyMcpState<R>) -> Self {
        Self {
            state: Arc::new(McpState { registry }),
            alloy: Some(Arc::new(alloy)),
            enabled_tools: None,
            access_context: None,
            runtime_binding: None,
            session_context: Arc::new(McpSessionContext::default()),
            audit_sink: None,
        }
    }

    pub fn with_alloy_and_enabled_tools(
        registry: ModuleRegistry,
        alloy: AlloyMcpState<R>,
        enabled_tools: HashSet<String>,
    ) -> Self {
        Self {
            state: Arc::new(McpState { registry }),
            alloy: Some(Arc::new(alloy)),
            enabled_tools: Some(Arc::new(enabled_tools)),
            access_context: None,
            runtime_binding: None,
            session_context: Arc::new(McpSessionContext::default()),
            audit_sink: None,
        }
    }

    pub fn with_access_context(mut self, access_context: McpAccessContext) -> Self {
        self.access_context = Some(Arc::new(access_context));
        self
    }

    pub fn with_runtime_binding(mut self, runtime_binding: McpRuntimeBinding) -> Self {
        self.access_context = Some(Arc::new(runtime_binding.access_context.clone()));
        self.runtime_binding = Some(Arc::new(runtime_binding));
        self
    }

    pub fn with_session_context(mut self, session_context: McpSessionContext) -> Self {
        self.session_context = Arc::new(session_context);
        self
    }

    pub fn with_audit_sink<T>(mut self, audit_sink: Arc<T>) -> Self
    where
        T: crate::runtime::McpAuditSink + 'static,
    {
        self.audit_sink = Some(audit_sink);
        self
    }

    pub fn with_shared_audit_sink(mut self, audit_sink: SharedMcpAuditSink) -> Self {
        self.audit_sink = Some(audit_sink);
        self
    }

    /// List all registered modules
    async fn list_modules_internal(&self) -> ModuleListResponse {
        list_modules(&self.state).await
    }

    /// Check if a module exists by slug
    async fn module_exists_internal(&self, slug: &str) -> ModuleLookupResponse {
        module_exists(
            &self.state,
            ModuleLookupRequest {
                slug: slug.to_string(),
            },
        )
        .await
    }

    /// Fetch module details by slug
    async fn module_details_internal(&self, slug: &str) -> ModuleDetailsResponse {
        module_details(
            &self.state,
            ModuleLookupRequest {
                slug: slug.to_string(),
            },
        )
        .await
    }

    /// Filter modules with pagination
    async fn list_modules_filtered_internal(
        &self,
        request: ModuleQueryRequest,
    ) -> ModuleListResponse {
        list_modules_filtered(&self.state, request).await
    }

    /// Fetch module details by static slug
    fn module_details_by_slug_internal(&self, slug: &str) -> ModuleDetailsResponse {
        module_details_by_slug(&self.state, slug)
    }

    fn tool_enabled_by_legacy_config(&self, tool_name: &str) -> bool {
        if tool_name == TOOL_MCP_HEALTH {
            return true;
        }

        match &self.enabled_tools {
            Some(enabled) => enabled.contains(tool_name),
            None => true,
        }
    }

    fn tool_allowed(&self, tool_name: &str) -> bool {
        if tool_name == TOOL_MCP_HEALTH {
            return true;
        }

        if !self.tool_enabled_by_legacy_config(tool_name) {
            return false;
        }

        match &self.access_context {
            Some(access_context) => {
                access_context
                    .authorize_tool(&default_tool_requirement(tool_name))
                    .allowed
            }
            None => true,
        }
    }

    fn protocol_version() -> rmcp::model::ProtocolVersion {
        rmcp::model::ProtocolVersion::V_2024_11_05
    }

    fn protocol_version_string() -> String {
        serde_json::to_value(Self::protocol_version())
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_else(|| "2024-11-05".to_string())
    }

    fn health_response(&self, tool_count: usize) -> McpHealthResponse {
        let enabled_tools = match (&self.enabled_tools, &self.access_context) {
            (Some(tools), _) => Some(tools.iter().cloned().collect::<Vec<String>>()),
            (None, Some(access_context)) => access_context.whoami().allowed_tools,
            (None, None) => None,
        };

        McpHealthResponse {
            status: "ready".to_string(),
            protocol_version: Self::protocol_version_string(),
            tool_count,
            enabled_tools,
            access_mode: if self.access_context.is_some() {
                "policy".to_string()
            } else {
                "allow_all".to_string()
            },
            identity: self
                .access_context
                .as_ref()
                .and_then(|access_context| access_context.identity.clone()),
        }
    }

    fn available_tool_names(&self) -> Vec<&'static str> {
        let mut tools = vec![
            TOOL_LIST_MODULES,
            TOOL_QUERY_MODULES,
            TOOL_MODULE_EXISTS,
            TOOL_MODULE_DETAILS,
            TOOL_CONTENT_MODULE,
            TOOL_BLOG_MODULE,
            TOOL_FORUM_MODULE,
            TOOL_PAGES_MODULE,
            TOOL_MCP_HEALTH,
            TOOL_MCP_WHOAMI,
        ];

        if self.alloy.is_some() {
            tools.extend_from_slice(ALL_ALLOY_TOOLS);
        }

        tools
            .into_iter()
            .filter(|name| self.tool_allowed(name))
            .collect()
    }

    fn serialize_response<T: serde::Serialize>(value: T) -> Result<String, rmcp::ErrorData> {
        serde_json::to_string(&value).map_err(|e| {
            rmcp::ErrorData::internal_error(format!("Failed to serialize response: {}", e), None)
        })
    }

    fn access_context_ref(&self) -> Option<&McpAccessContext> {
        self.access_context.as_deref()
    }

    fn runtime_binding_ref(&self) -> Option<&McpRuntimeBinding> {
        self.runtime_binding.as_deref()
    }

    fn scaffold_runtime_context(&self) -> McpScaffoldDraftRuntimeContext {
        McpScaffoldDraftRuntimeContext {
            session: (*self.session_context).clone(),
            runtime_binding: self.runtime_binding.as_deref().cloned(),
            access_context: self.access_context.as_deref().cloned(),
        }
    }

    async fn record_tool_allowed(&self, tool_name: &str) {
        self.record_tool_audit(McpToolCallAuditEvent::allowed(
            &self.session_context,
            self.access_context_ref(),
            self.runtime_binding_ref(),
            tool_name,
        ))
        .await;
    }

    async fn record_tool_denied(&self, tool_name: &str, reason: Option<String>) {
        self.record_tool_audit(McpToolCallAuditEvent::denied(
            &self.session_context,
            self.access_context_ref(),
            self.runtime_binding_ref(),
            tool_name,
            reason,
        ))
        .await;
    }

    async fn record_tool_audit(&self, event: McpToolCallAuditEvent) {
        if let Some(audit_sink) = &self.audit_sink {
            if let Err(error) = audit_sink.record_tool_call(event).await {
                tracing::warn!(error = %error, "Failed to persist MCP tool audit event");
            }
        }
    }
}

impl<R: ScriptRegistry + Send + Sync + 'static> ServerHandler for RusToKMcpServer<R> {
    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, rmcp::ErrorData> {
        tracing::info!(tool = %request.name, "MCP tool call");

        if !self.tool_enabled_by_legacy_config(request.name.as_ref()) {
            self.record_tool_denied(
                request.name.as_ref(),
                Some("Tool is disabled by configuration".to_string()),
            )
            .await;
            let content = Self::serialize_response(McpToolResponse::<()>::error(
                "tool_disabled",
                "Tool is disabled by configuration",
            ))?;
            return Ok(CallToolResult::success(vec![
                rmcp::model::ContentBlock::text(content),
            ]));
        }

        if request.name.as_ref() != TOOL_MCP_HEALTH {
            if let Some(access_context) = &self.access_context {
                let decision =
                    access_context.authorize_tool(&default_tool_requirement(request.name.as_ref()));
                if !decision.allowed {
                    self.record_tool_denied(
                        request.name.as_ref(),
                        decision.message.clone().or_else(|| decision.code.clone()),
                    )
                    .await;
                    let content = Self::serialize_response(McpToolResponse::<()>::error(
                        decision
                            .code
                            .clone()
                            .unwrap_or_else(|| "access_denied".to_string()),
                        decision
                            .message
                            .clone()
                            .unwrap_or_else(|| "MCP access policy denied this tool".to_string()),
                    ))?;
                    return Ok(CallToolResult::success(vec![
                        rmcp::model::ContentBlock::text(content),
                    ]));
                }
            }
        }

        self.record_tool_allowed(request.name.as_ref()).await;

        match request.name.as_ref() {
            TOOL_LIST_MODULES => {
                let result = self.list_modules_internal().await;
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_QUERY_MODULES => {
                let args = request
                    .arguments
                    .ok_or_else(|| rmcp::ErrorData::invalid_params("Missing arguments", None))?;
                let req: ModuleQueryRequest =
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        rmcp::ErrorData::invalid_params(format!("Invalid arguments: {}", e), None)
                    })?;
                let result = self.list_modules_filtered_internal(req).await;
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_MODULE_EXISTS => {
                let args = request
                    .arguments
                    .ok_or_else(|| rmcp::ErrorData::invalid_params("Missing arguments", None))?;
                let req: ModuleLookupRequest =
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        rmcp::ErrorData::invalid_params(format!("Invalid arguments: {}", e), None)
                    })?;
                let result = self.module_exists_internal(&req.slug).await;
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_MODULE_DETAILS => {
                let args = request
                    .arguments
                    .ok_or_else(|| rmcp::ErrorData::invalid_params("Missing arguments", None))?;
                let req: ModuleLookupRequest =
                    serde_json::from_value(serde_json::Value::Object(args)).map_err(|e| {
                        rmcp::ErrorData::invalid_params(format!("Invalid arguments: {}", e), None)
                    })?;
                let result = self.module_details_internal(&req.slug).await;
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_CONTENT_MODULE => {
                let result = self.module_details_by_slug_internal(MODULE_CONTENT);
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_BLOG_MODULE => {
                let result = self.module_details_by_slug_internal(MODULE_BLOG);
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_FORUM_MODULE => {
                let result = self.module_details_by_slug_internal(MODULE_FORUM);
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_PAGES_MODULE => {
                let result = self.module_details_by_slug_internal(MODULE_PAGES);
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_MCP_HEALTH => {
                let tool_count = self.available_tool_names().len();
                let result = self.health_response(tool_count);
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }
            TOOL_MCP_WHOAMI => {
                let result = self
                    .access_context
                    .as_ref()
                    .map(|access_context| access_context.whoami())
                    .unwrap_or_else(McpWhoAmIResponse::anonymous);
                let content = Self::serialize_response(McpToolResponse::success(result))?;
                Ok(CallToolResult::success(vec![
                    rmcp::model::ContentBlock::text(content),
                ]))
            }

            // ── Alloy tools ──────────────────────────────────────────────────────────
            name if ALL_ALLOY_TOOLS.contains(&name) => {
                let alloy = match &self.alloy {
                    Some(a) => Arc::clone(a),
                    None => {
                        let content = Self::serialize_response(McpToolResponse::<()>::error(
                            "not_configured",
                            "Alloy scripting is not configured in this MCP server",
                        ))?;
                        return Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]));
                    }
                };

                match name {
                    TOOL_ALLOY_LIST_SCRIPTS => {
                        let req: ListScriptsRequest = parse_optional_args(request.arguments)?;
                        let result = alloy_list_scripts(&alloy, req).await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_GET_SCRIPT => {
                        let args = require_args(request.arguments)?;
                        let req: GetScriptRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_get_script(&alloy, req).await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_CREATE_SCRIPT => {
                        let args = require_args(request.arguments)?;
                        let req: CreateScriptRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_create_script(&alloy, req).await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_UPDATE_SCRIPT => {
                        let args = require_args(request.arguments)?;
                        let req: UpdateScriptRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_update_script(&alloy, req).await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_DELETE_SCRIPT => {
                        let args = require_args(request.arguments)?;
                        let req: DeleteScriptRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_delete_script(&alloy, req).await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_VALIDATE_SCRIPT => {
                        let args = require_args(request.arguments)?;
                        let req: ValidateScriptRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_validate_script(&alloy, req);
                        let content = Self::serialize_response(McpToolResponse::success(result))?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_RUN_SCRIPT => {
                        let args = require_args(request.arguments)?;
                        let req: RunScriptRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_run_script(&alloy, req).await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_SCAFFOLD_MODULE => {
                        let args = require_args(request.arguments)?;
                        let req: ScaffoldModuleRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_scaffold_module(
                            &alloy,
                            Some(self.scaffold_runtime_context()),
                            req,
                        )
                        .await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD => {
                        let args = require_args(request.arguments)?;
                        let req: ReviewModuleScaffoldRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_review_module_scaffold(
                            &alloy,
                            Some(self.scaffold_runtime_context()),
                            req,
                        )
                        .await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD => {
                        let args = require_args(request.arguments)?;
                        let req: ApplyModuleScaffoldRequest = serde_json::from_value(
                            serde_json::Value::Object(args),
                        )
                        .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None))?;
                        let result = alloy_apply_module_scaffold(
                            &alloy,
                            Some(self.scaffold_runtime_context()),
                            req,
                        )
                        .await;
                        let content = Self::serialize_response(match result {
                            Ok(v) => McpToolResponse::success(v),
                            Err(e) => McpToolResponse::error("alloy_error", e),
                        })?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_LIST_ENTITY_TYPES => {
                        let result = alloy_list_entity_types();
                        let content = Self::serialize_response(McpToolResponse::success(result))?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    TOOL_ALLOY_SCRIPT_HELPERS => {
                        let result = alloy_script_helpers();
                        let content = Self::serialize_response(McpToolResponse::success(result))?;
                        Ok(CallToolResult::success(vec![
                            rmcp::model::ContentBlock::text(content),
                        ]))
                    }
                    _ => unreachable!("ALL_ALLOY_TOOLS exhausted"),
                }
            }

            _ => Err(rmcp::ErrorData::new(
                rmcp::model::ErrorCode::METHOD_NOT_FOUND,
                format!("Unknown tool: {}", request.name),
                None,
            )),
        }
    }

    async fn list_tools(
        &self,
        _request: Option<rmcp::model::PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, rmcp::ErrorData> {
        use rmcp::model::Tool;
        use schemars::schema_for;

        let empty_schema = match serde_json::to_value(schema_for!(())) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        };

        let module_exists_schema =
            match serde_json::to_value(schema_for!(crate::tools::ModuleLookupRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let module_query_schema =
            match serde_json::to_value(schema_for!(crate::tools::ModuleQueryRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let list_scripts_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::ListScriptsRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let get_script_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::GetScriptRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let create_script_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::CreateScriptRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let update_script_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::UpdateScriptRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let delete_script_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::DeleteScriptRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let validate_script_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::ValidateScriptRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let run_script_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::RunScriptRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let scaffold_module_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::ScaffoldModuleRequest)) {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };
        let review_scaffold_schema = match serde_json::to_value(schema_for!(
            crate::alloy_tools::ReviewModuleScaffoldRequest
        )) {
            Ok(serde_json::Value::Object(map)) => map,
            _ => serde_json::Map::new(),
        };
        let apply_scaffold_schema =
            match serde_json::to_value(schema_for!(crate::alloy_tools::ApplyModuleScaffoldRequest))
            {
                Ok(serde_json::Value::Object(map)) => map,
                _ => serde_json::Map::new(),
            };

        let mut tools = vec![
            Tool::new(
                TOOL_LIST_MODULES,
                "List all registered RusToK modules with their metadata",
                empty_schema.clone(),
            ),
            Tool::new(
                TOOL_QUERY_MODULES,
                "List modules with filters and pagination",
                module_query_schema,
            ),
            Tool::new(
                TOOL_MODULE_EXISTS,
                "Check if a module exists by its slug",
                module_exists_schema.clone(),
            ),
            Tool::new(
                TOOL_MODULE_DETAILS,
                "Fetch module metadata by slug",
                module_exists_schema,
            ),
            Tool::new(
                TOOL_CONTENT_MODULE,
                "Fetch content module metadata",
                empty_schema.clone(),
            ),
            Tool::new(
                TOOL_BLOG_MODULE,
                "Fetch blog module metadata",
                empty_schema.clone(),
            ),
            Tool::new(
                TOOL_FORUM_MODULE,
                "Fetch forum module metadata",
                empty_schema.clone(),
            ),
            Tool::new(
                TOOL_PAGES_MODULE,
                "Fetch pages module metadata",
                empty_schema.clone(),
            ),
            Tool::new(
                TOOL_MCP_HEALTH,
                "MCP readiness and configuration status",
                empty_schema.clone(),
            ),
            Tool::new(
                TOOL_MCP_WHOAMI,
                "Inspect the current MCP identity, permissions, scopes, and tool policy",
                empty_schema.clone(),
            ),
        ];

        if self.alloy.is_some() {
            tools.extend([
                Tool::new(
                    TOOL_ALLOY_LIST_SCRIPTS,
                    "List Alloy scripts with optional status filter",
                    list_scripts_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_GET_SCRIPT,
                    "Get a single Alloy script by name or UUID",
                    get_script_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_CREATE_SCRIPT,
                    "Create a new Alloy Rhai script",
                    create_script_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_UPDATE_SCRIPT,
                    "Update an existing Alloy script (code, description, status)",
                    update_script_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_DELETE_SCRIPT,
                    "Delete an Alloy script by UUID",
                    delete_script_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_VALIDATE_SCRIPT,
                    "Validate Rhai script syntax without executing",
                    validate_script_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_RUN_SCRIPT,
                    "Execute an Alloy script manually with optional params and entity context",
                    run_script_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_SCAFFOLD_MODULE,
                    "Stage a reviewed draft RusToK module crate scaffold without writing it into the workspace yet",
                    scaffold_module_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD,
                    "Fetch a staged Alloy module scaffold draft for review before apply",
                    review_scaffold_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
                    "Apply a reviewed Alloy module scaffold draft into the workspace with explicit confirmation",
                    apply_scaffold_schema,
                ),
                Tool::new(
                    TOOL_ALLOY_LIST_ENTITY_TYPES,
                    "List all known entity types in the platform",
                    empty_schema.clone(),
                ),
                Tool::new(
                    TOOL_ALLOY_SCRIPT_HELPERS,
                    "List available Rhai helper functions with signatures and descriptions",
                    empty_schema,
                ),
            ]);
        }

        tools.retain(|tool| self.tool_allowed(tool.name.as_ref()));

        Ok(ListToolsResult {
            tools,
            next_cursor: None,
            meta: None,
        })
    }

    fn get_info(&self) -> ServerInfo {
        let mut server_info = Implementation::default();
        server_info.name = "RusToK MCP Server".to_string();
        server_info.version = env!("CARGO_PKG_VERSION").to_string();
        server_info.title = Some("RusToK MCP Server".to_string());
        server_info.description = Some(
            "MCP server for exploring RusToK modules, introspecting MCP identity/policy, managing Alloy scripts, and staging/reviewing/applying draft RusToK module scaffolds. Use mcp_whoami for access context and alloy_* tools for Alloy capabilities.".to_string(),
        );

        let mut info = ServerInfo::default();
        info.protocol_version = Self::protocol_version();
        info.capabilities = rmcp::model::ServerCapabilities::default();
        info.server_info = server_info;
        info.instructions = Some(
            "MCP server for RusToK. Use mcp_whoami for access context, list_modules/module_exists for module discovery, and alloy_* tools for script management plus staged draft module scaffolding with explicit review/apply.".to_string(),
        );

        info
    }
}

/// Serve the MCP server over stdio
pub async fn serve_stdio(config: McpServerConfig) -> Result<()> {
    let McpServerConfig {
        registry,
        enabled_tools,
        access_context,
        session_context,
        access_resolver,
        audit_sink,
    } = config;

    let runtime_binding = if let Some(access_context) = access_context {
        Some(McpRuntimeBinding::from_access_context(access_context))
    } else if let Some(access_resolver) = access_resolver {
        Some(
            access_resolver
                .resolve_runtime_binding(&session_context)
                .await?,
        )
    } else {
        None
    };

    let mut server = if let Some(enabled_tools) = enabled_tools {
        RusToKMcpServer::with_enabled_tools(registry, enabled_tools)
    } else {
        RusToKMcpServer::new(registry)
    };

    server = server.with_session_context(session_context);

    if let Some(runtime_binding) = runtime_binding {
        server = server.with_runtime_binding(runtime_binding);
    }

    if let Some(audit_sink) = audit_sink {
        server = server.with_shared_audit_sink(audit_sink);
    }

    server
        .serve(stdio())
        .await
        .map_err(|e| anyhow::anyhow!("MCP server error: {}", e))?
        .waiting()
        .await
        .map(|_| ())
        .map_err(|e| anyhow::anyhow!("MCP server error: {}", e))
}

fn require_args(
    args: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<serde_json::Map<String, serde_json::Value>, rmcp::ErrorData> {
    args.ok_or_else(|| rmcp::ErrorData::invalid_params("Missing arguments", None))
}

fn parse_optional_args<T: serde::de::DeserializeOwned + Default>(
    args: Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<T, rmcp::ErrorData> {
    match args {
        Some(map) => serde_json::from_value(serde_json::Value::Object(map))
            .map_err(|e| rmcp::ErrorData::invalid_params(e.to_string(), None)),
        None => Ok(T::default()),
    }
}
