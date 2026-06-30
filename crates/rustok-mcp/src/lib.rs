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
pub mod alloy_tools;
pub mod management;
pub mod runtime;
pub mod server;
pub mod tools;

pub use access::{
    default_tool_requirement, McpAccessContext, McpAccessPolicy, McpActorType,
    McpAuthorizationDecision, McpIdentity, McpToolRequirement, McpWhoAmIResponse,
};
pub use alloy_scaffold::{
    apply_staged_scaffold, generate_module_scaffold, ApplyModuleScaffoldRequest,
    ApplyModuleScaffoldResponse, ModuleScaffoldDraftStatus, ReviewModuleScaffoldRequest,
    ReviewModuleScaffoldResponse, ScaffoldModuleFile, ScaffoldModulePreview, ScaffoldModuleRequest,
    StageModuleScaffoldResponse, StagedModuleScaffold,
};
pub use alloy_tools::{
    AlloyMcpState, AlloyScriptInfo, ALL_ALLOY_TOOLS, TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
    TOOL_ALLOY_CREATE_SCRIPT, TOOL_ALLOY_DELETE_SCRIPT, TOOL_ALLOY_GET_SCRIPT,
    TOOL_ALLOY_LIST_ENTITY_TYPES, TOOL_ALLOY_LIST_SCRIPTS, TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD,
    TOOL_ALLOY_RUN_SCRIPT, TOOL_ALLOY_SCAFFOLD_MODULE, TOOL_ALLOY_SCRIPT_HELPERS,
    TOOL_ALLOY_UPDATE_SCRIPT, TOOL_ALLOY_VALIDATE_SCRIPT,
};
pub use management::{
    CreateMcpClientCommand, McpClientMutationRecord, McpManagementMutationContext,
    McpManagementMutationError, McpManagementMutationPort, McpManagementMutationRuntime,
    McpPolicyMutationRecord, McpTokenSecretResult, RotateMcpTokenCommand, UpdateMcpPolicyCommand,
};
pub use runtime::{
    McpAccessResolver, McpAuditSink, McpRuntimeBinding, McpScaffoldDraftRuntimeContext,
    McpScaffoldDraftStore, McpSessionContext, McpToolCallAuditEvent, McpToolCallOutcome,
    SharedMcpAccessResolver, SharedMcpAuditSink, SharedMcpScaffoldDraftStore,
};
pub use server::{serve_stdio, McpServerConfig, RusToKMcpServer};
pub use tools::{
    McpHealthResponse, McpState, McpToolError, McpToolResponse, ModuleDetailsResponse, ModuleInfo,
    ModuleListResponse, ModuleLookupRequest, ModuleLookupResponse, ModuleQueryRequest, MODULE_BLOG,
    MODULE_CONTENT, MODULE_FORUM, MODULE_PAGES, TOOL_BLOG_MODULE, TOOL_CONTENT_MODULE,
    TOOL_FORUM_MODULE, TOOL_LIST_MODULES, TOOL_MCP_HEALTH, TOOL_MCP_WHOAMI, TOOL_MODULE_DETAILS,
    TOOL_MODULE_EXISTS, TOOL_PAGES_MODULE, TOOL_QUERY_MODULES,
};

#[cfg(test)]
mod contract_tests;
