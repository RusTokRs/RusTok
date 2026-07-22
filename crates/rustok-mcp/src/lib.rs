/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! RusToK MCP Server
//!
//! This crate provides a Model Context Protocol (MCP) server for exploring
//! and interacting with RusToK modules, including Alloy scripting management.

pub mod access;
mod alloy_scaffold;
#[path = "alloy_tools.rs"]
mod alloy_tools_unchecked;
pub mod alloy_tools {
    use alloy::storage::ScriptRegistry;

    pub use super::alloy_tools_unchecked::*;

    pub async fn alloy_apply_module_scaffold<R: ScriptRegistry>(
        state: &AlloyMcpState<R>,
        context: Option<crate::McpScaffoldDraftRuntimeContext>,
        request: ApplyModuleScaffoldRequest,
    ) -> Result<ApplyModuleScaffoldResponse, String> {
        super::scaffold_workspace::apply_authorized_module_scaffold(state, context, request).await
    }
}
#[cfg(feature = "graphql")]
pub mod graphql;
pub mod management;
pub mod runtime;
pub mod scaffold_workspace;
pub mod server;
pub mod tools;

pub use access::{
    McpAccessContext, McpAccessPolicy, McpActorType, McpAuthorizationDecision, McpIdentity,
    McpToolRequirement, McpWhoAmIResponse, default_tool_requirement,
};
pub use alloy_scaffold::{
    ApplyModuleScaffoldRequest, ApplyModuleScaffoldResponse, ModuleScaffoldDraftStatus,
    ReviewModuleScaffoldRequest, ReviewModuleScaffoldResponse, ScaffoldModuleFile,
    ScaffoldModulePreview, ScaffoldModuleRequest, StageModuleScaffoldResponse,
    StagedModuleScaffold, apply_staged_scaffold, generate_module_scaffold,
};
pub use alloy_tools::{
    ALL_ALLOY_TOOLS, AlloyMcpState, AlloyScriptInfo, TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
    TOOL_ALLOY_CREATE_SCRIPT, TOOL_ALLOY_DELETE_SCRIPT, TOOL_ALLOY_GET_SCRIPT,
    TOOL_ALLOY_LIST_ENTITY_TYPES, TOOL_ALLOY_LIST_SCRIPTS, TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD,
    TOOL_ALLOY_RUN_SCRIPT, TOOL_ALLOY_SCAFFOLD_MODULE, TOOL_ALLOY_SCRIPT_HELPERS,
    TOOL_ALLOY_UPDATE_SCRIPT, TOOL_ALLOY_VALIDATE_SCRIPT,
};
pub use management::{
    ApplyMcpModuleScaffoldDraftRequest, ApplyMcpScaffoldDraftCommand,
    BootstrapMcpRemoteSessionRequest, CreateMcpClientCommand, CreateMcpClientRequest,
    CreateMcpClientResponse, McpAuditEventRecord, McpAuditEventResponse, McpAuditQuery,
    McpClientDetailsRecord, McpClientDetailsResponse, McpClientRecord, McpClientSummaryResponse,
    McpManagementContext, McpManagementMutationError, McpManagementPort, McpManagementRuntime,
    McpModuleScaffoldDraftResponse, McpPolicyRecord, McpPolicyResponse, McpRemoteToolCallRequest,
    McpRemoteToolCallResponse, McpScaffoldDraftRecord, McpTokenRecord, McpTokenResponse,
    McpTokenSecretResult, RotateMcpTokenCommand, RotateMcpTokenRequest, RotateMcpTokenResponse,
    StageMcpModuleScaffoldDraftRequest, StageMcpScaffoldDraftCommand, UpdateMcpPolicyCommand,
    UpdateMcpPolicyRequest,
};
pub use runtime::{
    McpAccessResolver, McpAuditSink, McpRuntimeBinding, McpScaffoldDraftRuntimeContext,
    McpScaffoldDraftStore, McpSessionContext, McpToolCallAuditEvent, McpToolCallOutcome,
    SharedMcpAccessResolver, SharedMcpAuditSink, SharedMcpScaffoldDraftStore,
};
pub use scaffold_workspace::{
    MCP_SCAFFOLD_WORKSPACE_ROOT_ENV, apply_authorized_module_scaffold, authorize_scaffold_workspace,
};
pub use server::{McpServerConfig, RusToKMcpServer, serve_stdio};
pub use tools::{
    MODULE_BLOG, MODULE_CONTENT, MODULE_FORUM, MODULE_PAGES, McpHealthResponse, McpState,
    McpToolError, McpToolResponse, ModuleDetailsResponse, ModuleInfo, ModuleListResponse,
    ModuleLookupRequest, ModuleLookupResponse, ModuleQueryRequest, TOOL_BLOG_MODULE,
    TOOL_CONTENT_MODULE, TOOL_FORUM_MODULE, TOOL_LIST_MODULES, TOOL_MCP_HEALTH, TOOL_MCP_WHOAMI,
    TOOL_MODULE_DETAILS, TOOL_MODULE_EXISTS, TOOL_PAGES_MODULE, TOOL_QUERY_MODULES,
};

#[cfg(test)]
mod contract_tests;
