#!/usr/bin/env node
// RusTok MCP admin UI ownership and FFA boundary guardrails.

import { existsSync, readFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = process.env.RUSTOK_VERIFY_REPO_ROOT
  ? path.resolve(process.env.RUSTOK_VERIFY_REPO_ROOT)
  : path.resolve(scriptDir, "../..");
const failures = [];

function repoPath(relativePath) {
  return path.join(repoRoot, relativePath);
}

function readRepo(relativePath) {
  return readFileSync(repoPath(relativePath), "utf8");
}

function fail(message) {
  failures.push(message);
}

function assertExists(relativePath, description) {
  if (!existsSync(repoPath(relativePath))) fail(description);
}

function assertContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (!found) fail(description);
}

function assertNotContains(text, pattern, description) {
  const found = typeof pattern === "string" ? text.includes(pattern) : pattern.test(text);
  if (found) fail(description);
}

const nextPackagePath = "apps/next-admin/packages/rustok-mcp/src/index.tsx";
const nextRoutePath = "apps/next-admin/src/app/dashboard/mcp/page.tsx";
const leptosHostCargoPath = "apps/admin/Cargo.toml";
const leptosHostRouterPath = "apps/admin/src/app/router.rs";
const leptosLibPath = "crates/rustok-mcp/admin/src/lib.rs";
const leptosUiPath = "crates/rustok-mcp/admin/src/ui/leptos.rs";
const transportModPath = "crates/rustok-mcp/admin/src/transport/mod.rs";
const nativeAdapterPath = "crates/rustok-mcp/admin/src/transport/native_server_adapter.rs";
const graphqlAdapterPath = "crates/rustok-mcp/admin/src/transport/graphql_adapter.rs";
const managementContractPath = "crates/rustok-mcp/src/management.rs";
const mcpAccessContractPath = "crates/rustok-mcp/src/access.rs";
const serverMcpControllerPath = "apps/server/src/controllers/mcp.rs";
const managementProviderPath = "apps/server/src/services/mcp_management_mutation_provider.rs";
const runtimeRegistrationPath = "apps/server/src/services/module_event_dispatcher.rs";
const aiNextPackagePath = "apps/next-admin/packages/rustok-ai/src/index.tsx";
const aiLeptosUiPath = "crates/rustok-ai/admin/src/ui/leptos.rs";
const mcpPlanPath = "crates/rustok-mcp/docs/implementation-plan.md";
const centralRegistryPath = "docs/modules/registry.md";

for (const [file, description] of [
  [nextPackagePath, "expected MCP Next owner package entry"],
  [nextRoutePath, "expected thin MCP host route"],
  [leptosHostCargoPath, "expected MCP Leptos host dependency"],
  [leptosHostRouterPath, "expected MCP Leptos host route"],
  [leptosLibPath, "expected MCP Leptos admin crate root"],
  [leptosUiPath, "expected MCP Leptos UI adapter"],
  [transportModPath, "expected MCP transport facade"],
  [nativeAdapterPath, "expected MCP native server-function adapter"],
  [graphqlAdapterPath, "expected MCP GraphQL adapter"],
  [managementContractPath, "expected MCP management port contract"],
  [mcpAccessContractPath, "expected MCP access contract"],
  [serverMcpControllerPath, "expected server MCP controller adapter"],
  [managementProviderPath, "expected server MCP management provider"],
  [runtimeRegistrationPath, "expected host runtime provider registration"],
  [mcpPlanPath, "expected MCP implementation plan"],
  [centralRegistryPath, "expected central module registry"],
]) {
  assertExists(file, `${file}: ${description}`);
}

const nextPackage = readRepo(nextPackagePath);
const nextRoute = readRepo(nextRoutePath);
const leptosHostCargo = readRepo(leptosHostCargoPath);
const leptosHostRouter = readRepo(leptosHostRouterPath);
const leptosLib = readRepo(leptosLibPath);
const leptosUi = readRepo(leptosUiPath);
const transportMod = readRepo(transportModPath);
const nativeAdapter = readRepo(nativeAdapterPath);
const graphqlAdapter = readRepo(graphqlAdapterPath);
const managementContract = readRepo(managementContractPath);
const mcpAccessContract = readRepo(mcpAccessContractPath);
const serverMcpController = readRepo(serverMcpControllerPath);
const managementProvider = readRepo(managementProviderPath);
const runtimeRegistration = readRepo(runtimeRegistrationPath);
const aiNextPackage = readRepo(aiNextPackagePath);
const aiLeptosUi = readRepo(aiLeptosUiPath);
const mcpPlan = readRepo(mcpPlanPath);
const centralRegistry = readRepo(centralRegistryPath);

for (const marker of [
  "export function McpAdminPage",
  "const MCP_SCAFFOLD_DRAFTS_QUERY",
  "mcpModuleScaffoldDrafts(limit: 20)",
  "const MCP_AUDIT_EVENTS_QUERY",
  "mcpAuditEvents(limit: 30)",
  "const MCP_CLIENTS_QUERY",
  "mcpClients(limit: 50)",
  "const MCP_CLIENT_DETAILS_QUERY",
  "mcpClient(id: $id)",
  "const CREATE_MCP_CLIENT_MUTATION",
  "const ROTATE_MCP_TOKEN_MUTATION",
  "const UPDATE_MCP_POLICY_MUTATION",
  "const REVOKE_MCP_TOKEN_MUTATION",
  "const DEACTIVATE_MCP_CLIENT_MUTATION",
  "const STAGE_MCP_SCAFFOLD_DRAFT_MUTATION",
  "stageMcpModuleScaffoldDraft(input: $input)",
  "const APPLY_MCP_SCAFFOLD_DRAFT_MUTATION",
  "applyMcpModuleScaffoldDraft(draftId: $draftId, input: $input)",
]) {
  assertContains(nextPackage, marker, `${nextPackagePath}: expected Next MCP owner marker ${marker}`);
}

assertContains(nextRoute, "import { McpAdminPage } from '@rustok/mcp-admin';", `${nextRoutePath}: route must mount the MCP owner package`);
assertNotContains(nextRoute, "mcpModuleScaffoldDrafts", `${nextRoutePath}: host route must not own MCP draft GraphQL`);
assertContains(leptosHostCargo, "rustok-mcp-admin", `${leptosHostCargoPath}: host must depend on the MCP owner package`);
assertContains(leptosHostCargo, "rustok-mcp-admin/csr", `${leptosHostCargoPath}: CSR debug profile must include the MCP owner package`);
assertContains(leptosHostCargo, "rustok-mcp-admin/hydrate", `${leptosHostCargoPath}: hydrate profile must include the MCP owner package`);
assertContains(leptosHostCargo, "rustok-mcp-admin/ssr", `${leptosHostCargoPath}: SSR profile must include the MCP owner package`);
assertContains(leptosHostRouter, 'path=path!("/mcp") view=rustok_mcp_admin::McpAdmin', `${leptosHostRouterPath}: host route must mount the MCP owner component`);
assertNotContains(leptosHostRouter, "mcpModuleScaffoldDrafts", `${leptosHostRouterPath}: Leptos host route must not own MCP transport logic`);

assertContains(leptosLib, "pub use ui::McpAdmin;", `${leptosLibPath}: Leptos crate must re-export owner UI`);
assertContains(leptosUi, "transport::fetch_scaffold_drafts", `${leptosUiPath}: UI must call transport facade`);
assertContains(leptosUi, "transport::fetch_audit_events", `${leptosUiPath}: UI must expose owner-owned audit events`);
assertContains(leptosUi, "transport::fetch_clients", `${leptosUiPath}: UI must expose owner-owned MCP clients`);
assertContains(leptosUi, "transport::fetch_client_details", `${leptosUiPath}: UI must expose owner-owned policy/token details`);
assertContains(transportMod, "pub mod native_server_adapter;", `${transportModPath}: transport facade must wire native adapter`);
assertContains(transportMod, "pub mod graphql_adapter;", `${transportModPath}: transport facade must wire GraphQL adapter`);
assertContains(nativeAdapter, "#[server", `${nativeAdapterPath}: native adapter must expose server functions for Leptos FFA`);
assertContains(nativeAdapter, "mcp_scaffold_drafts_native", `${nativeAdapterPath}: native adapter must own drafts endpoint`);
for (const marker of [
  "mcp_native_context",
  "mcp_audit_events_native",
  "mcp_clients_native",
  "mcp_client_details_native",
  "mcp_create_client_native",
  "mcp_rotate_token_native",
  "mcp_update_policy_native",
  "mcp_revoke_token_native",
  "mcp_deactivate_client_native",
  "McpManagementRuntime",
  "ensure_mcp_read",
  "SELECT id, client_id, actor_id, actor_type, action, outcome",
  "SELECT id, slug, display_name, description, actor_type, is_active",
  "SELECT allowed_tools, denied_tools, granted_permissions, granted_scopes",
  "SELECT id, token_name, token_preview, last_used_at, expires_at, revoked_at",
  "ensure_mcp_manage",
  "mcp:manage required",
  ".stage_scaffold_draft(",
  ".apply_scaffold_draft(",
  "StageMcpScaffoldDraftCommand",
  "ApplyMcpScaffoldDraftCommand",
]) {
  assertContains(nativeAdapter, marker, `${nativeAdapterPath}: native adapter must keep DB-backed runtime marker ${marker}`);
}
for (const marker of [
  "generate_module_scaffold",
  "apply_staged_scaffold",
  "INSERT INTO mcp_scaffold_drafts",
  "UPDATE mcp_scaffold_drafts",
  "INSERT INTO mcp_audit_logs",
]) {
  assertNotContains(
    nativeAdapter,
    marker,
    `${nativeAdapterPath}: owner UI adapter must delegate scaffold writes through McpManagementPort (${marker})`,
  );
}
assertNotContains(
  nativeAdapter,
  "must be wired by the host runtime",
  `${nativeAdapterPath}: native server functions must not remain host-runtime stubs`,
);
assertContains(graphqlAdapter, "pub const MCP_SCAFFOLD_DRAFTS_QUERY", `${graphqlAdapterPath}: GraphQL adapter must expose drafts query`);
assertContains(graphqlAdapter, "pub fn scaffold_drafts_request", `${graphqlAdapterPath}: GraphQL adapter must expose request builder`);
assertContains(graphqlAdapter, "pub const MCP_AUDIT_EVENTS_QUERY", `${graphqlAdapterPath}: GraphQL adapter must expose audit query`);
assertContains(graphqlAdapter, "pub fn audit_events_request", `${graphqlAdapterPath}: GraphQL adapter must expose audit request builder`);
assertContains(graphqlAdapter, "pub const MCP_CLIENTS_QUERY", `${graphqlAdapterPath}: GraphQL adapter must expose clients query`);
assertContains(graphqlAdapter, "pub const MCP_CLIENT_DETAILS_QUERY", `${graphqlAdapterPath}: GraphQL adapter must expose client details query`);
assertContains(graphqlAdapter, "pub fn clients_request", `${graphqlAdapterPath}: GraphQL adapter must expose clients request builder`);
assertContains(graphqlAdapter, "pub fn client_details_request", `${graphqlAdapterPath}: GraphQL adapter must expose details request builder`);
for (const marker of [
  "pub const CREATE_MCP_CLIENT_MUTATION",
  "pub const ROTATE_MCP_TOKEN_MUTATION",
  "pub const UPDATE_MCP_POLICY_MUTATION",
  "pub const REVOKE_MCP_TOKEN_MUTATION",
  "pub const DEACTIVATE_MCP_CLIENT_MUTATION",
  "pub fn create_client_request",
  "pub fn rotate_token_request",
  "pub fn update_policy_request",
  "pub fn revoke_token_request",
  "pub fn deactivate_client_request",
]) {
  assertContains(graphqlAdapter, marker, `${graphqlAdapterPath}: expected management GraphQL marker ${marker}`);
}
for (const marker of ["leptos::", "#[component]", "#[server]", "RwSignal", "LocalResource", "web_sys::"]) {
  assertNotContains(graphqlAdapter, marker, `${graphqlAdapterPath}: GraphQL adapter must stay UI/runtime free (${marker})`);
}

for (const marker of [
  "trait McpManagementPort",
  "struct McpManagementRuntime",
  "CreateMcpClientCommand",
  "RotateMcpTokenCommand",
  "UpdateMcpPolicyCommand",
  "StageMcpScaffoldDraftCommand",
  "ApplyMcpScaffoldDraftCommand",
  "McpScaffoldDraftRecord",
  "pub struct BootstrapMcpRemoteSessionRequest",
  "pub struct McpRemoteToolCallRequest",
  "pub struct McpRemoteToolCallResponse",
  "pub struct CreateMcpClientRequest",
  "pub struct RotateMcpTokenRequest",
  "pub struct UpdateMcpPolicyRequest",
  "pub struct McpAuditQuery",
  "pub struct StageMcpModuleScaffoldDraftRequest",
  "pub struct ApplyMcpModuleScaffoldDraftRequest",
  "pub struct McpClientSummaryResponse",
  "pub struct McpClientDetailsResponse",
  "pub struct McpAuditEventResponse",
  "pub struct McpModuleScaffoldDraftResponse",
]) {
  assertContains(managementContract, marker, `${managementContractPath}: expected owner management contract ${marker}`);
}
assertNotContains(managementContract, "sea_orm", `${managementContractPath}: owner port contract must remain persistence-free`);
assertContains(mcpAccessContract, "impl FromStr for McpActorType", `${mcpAccessContractPath}: actor parsing must stay owner-owned`);
for (const marker of [
  "pub struct BootstrapMcpRemoteSessionRequest",
  "pub struct McpRemoteToolCallRequest",
  "pub struct McpRemoteToolCallResponse",
  "pub struct CreateMcpClientRequest",
  "pub struct RotateMcpTokenRequest",
  "pub struct UpdateMcpPolicyRequest",
  "pub struct McpAuditQuery",
  "pub struct StageMcpModuleScaffoldDraftRequest",
  "pub struct ApplyMcpModuleScaffoldDraftRequest",
  "pub struct McpClientSummaryResponse",
  "pub struct McpClientDetailsResponse",
  "pub struct McpAuditEventResponse",
  "pub struct McpModuleScaffoldDraftResponse",
]) {
  assertNotContains(
    serverMcpController,
    marker,
    `${serverMcpControllerPath}: REST/control-plane DTO ownership must stay in rustok-mcp (${marker})`,
  );
}
assertContains(serverMcpController, "use rustok_mcp::{", `${serverMcpControllerPath}: controller must import owner MCP contracts`);
assertContains(serverMcpController, "CreateMcpClientRequest", `${serverMcpControllerPath}: controller must consume owner request DTOs`);
assertContains(serverMcpController, "McpClientSummaryResponse", `${serverMcpControllerPath}: controller must consume owner response DTOs`);
assertContains(serverMcpController, "McpActorType::from_str", `${serverMcpControllerPath}: controller must delegate actor parsing to rustok-mcp`);
assertNotContains(serverMcpController, '"human_user" => Ok(McpActorType::HumanUser)', `${serverMcpControllerPath}: actor parsing must not be duplicated in server`);
assertContains(managementProvider, "impl McpManagementPort", `${managementProviderPath}: server must implement MCP management port`);
assertContains(managementProvider, "McpManagementService::create_client", `${managementProviderPath}: provider must delegate to canonical management service`);
assertContains(managementProvider, "McpManagementService::rotate_token", `${managementProviderPath}: provider must preserve canonical token security logic`);
assertContains(managementProvider, "McpManagementService::stage_scaffold_draft", `${managementProviderPath}: provider must delegate scaffold staging to the canonical management service`);
assertContains(managementProvider, "McpManagementService::apply_scaffold_draft", `${managementProviderPath}: provider must preserve canonical scaffold claim/recovery logic`);
assertContains(runtimeRegistration, "McpManagementRuntime::new", `${runtimeRegistrationPath}: host must register MCP management runtime`);

for (const marker of [
  "MCP Alloy Drafts",
  "MCP_SCAFFOLD_DRAFTS_QUERY",
  "mcpModuleScaffoldDrafts",
  "stageMcpModuleScaffoldDraft",
  "applyMcpModuleScaffoldDraft",
  "MCP_CLIENTS_QUERY",
  "mcpClients(limit: 50)",
  "mcpClient(id: $id)",
  "createMcpClient(input: $input)",
  "rotateMcpClientToken",
  "updateMcpClientPolicy",
  "revokeMcpToken",
  "deactivateMcpClient",
]) {
  assertNotContains(aiNextPackage, marker, `${aiNextPackagePath}: MCP draft UI must not live in rustok-ai (${marker})`);
  assertNotContains(aiLeptosUi, marker, `${aiLeptosUiPath}: MCP draft UI must not live in rustok-ai (${marker})`);
}

assertContains(mcpPlan, "mcp_admin_owner_ui_slice", `${mcpPlanPath}: plan must record MCP owner UI checkpoint`);
assertContains(mcpPlan, "## FFA/FBA status", `${mcpPlanPath}: plan must include FFA/FBA status`);
assertContains(centralRegistry, "| `rustok-mcp` | admin + Next admin |", `${centralRegistryPath}: readiness board must include MCP admin UI row`);

if (failures.length > 0) {
  console.error("MCP admin boundary verification failed:");
  for (const failure of failures) console.error(`- ${failure}`);
  process.exit(1);
}

console.log("MCP admin boundary verification passed");
