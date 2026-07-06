# rustok-mcp / CRATE_API

## Public Modules
`access`, `alloy_tools`, `runtime`, `server`, `tools`.

## Primary Public Types and Signatures
- `pub async fn serve_stdio(config: McpServerConfig) -> Result<...>`
- `pub struct McpServerConfig`
- `pub struct RusToKMcpServer`
- `pub struct McpIdentity`
- `pub struct McpAccessContext`
- `pub struct McpAccessPolicy`
- `pub struct McpToolRequirement`
- `pub struct McpWhoAmIResponse`
- `pub struct McpSessionContext`
- `pub struct McpRuntimeBinding`
- `pub struct McpScaffoldDraftRuntimeContext`
- `pub struct McpToolCallAuditEvent`
- `pub struct ScaffoldModuleRequest`
- `pub struct ScaffoldModuleFile`
- `pub struct ScaffoldModulePreview`
- `pub struct StageModuleScaffoldResponse`
- `pub struct ReviewModuleScaffoldRequest`
- `pub struct ReviewModuleScaffoldResponse`
- `pub struct ApplyModuleScaffoldRequest`
- `pub struct ApplyModuleScaffoldResponse`
- `pub struct StagedModuleScaffold`
- `pub enum ModuleScaffoldDraftStatus`
- `pub fn generate_module_scaffold(request: &ScaffoldModuleRequest) -> Result<ScaffoldModulePreview, ...>`
- `pub fn apply_staged_scaffold(draft: &StagedModuleScaffold, workspace_root: &str) -> Result<ApplyModuleScaffoldResponse, ...>`
- `pub const TOOL_ALLOY_SCAFFOLD_MODULE: &str`
- `pub const TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD: &str`
- `pub const TOOL_ALLOY_APPLY_MODULE_SCAFFOLD: &str`
- `pub trait McpAccessResolver`
- `pub trait McpAuditSink`
- `pub trait McpScaffoldDraftStore`
- `pub trait McpManagementPort`
- `pub struct StageMcpScaffoldDraftCommand`
- `pub struct ApplyMcpScaffoldDraftCommand`
- `pub struct McpScaffoldDraftRecord`
- Public MCP tools from `tools::*` and `alloy_tools::*`.

## Events
- Publishes: N/A (RPC/MCP adapter).
- Consumes: MCP client commands/requests.

## Dependencies on Other RusToK Crates
- `rustok-core`

## Common AI Mistakes
- Confuses MCP runtime and the `rustok-mcp` crate.
- Considers `enabled_tools` a full authz model, though it is now only a compatibility shim.
- Documents MCP spec locally instead of referencing the official upstream.
- Considers `alloy_scaffold_module` as generating a ready production module, though it is only a staged draft scaffold.
- Tries to register a scaffold module in the runtime without its own permission surface in `rustok-core`.

## Minimum Contract Set

### Input DTOs/Commands
- Input contract is defined by the public DTOs/commands from the crate and corresponding `pub` exports in `src/lib.rs`.
- All changes to public DTO fields are considered breaking changes and require synchronized updates to transport adapters and MCP clients that depend on them.
- For the access layer, changes to `McpIdentity`, `McpAccessContext`, `McpAccessPolicy`, `McpToolRequirement`, `McpWhoAmIResponse`, `McpSessionContext`, `McpRuntimeBinding`, `McpToolCallAuditEvent` are also considered breaking.
- For the Alloy module scaffolding layer, changes to `ScaffoldModuleRequest`, `ScaffoldModulePreview`, `StageModuleScaffoldResponse`, `ReviewModuleScaffoldRequest`, `ReviewModuleScaffoldResponse`, `ApplyModuleScaffoldRequest`, `ApplyModuleScaffoldResponse`, `StagedModuleScaffold` and semantics of `TOOL_ALLOY_SCAFFOLD_MODULE` / `TOOL_ALLOY_REVIEW_MODULE_SCAFFOLD` / `TOOL_ALLOY_APPLY_MODULE_SCAFFOLD` are considered breaking.

### Domain Invariants
- Multi-tenant boundary invariants (tenant/resource isolation, auth context) are considered a mandatory part of the contract.
- Tool authorization in `rustok-mcp` first checks the coarse-grained legacy allow-list, then MCP access policy/permissions/scopes.
- Persisted MCP auth binding is performed at session start via `McpAccessResolver`; `rustok-mcp` does not pull server-specific ORM/runtime code into itself.
- Persisted Alloy draft flow may be connected via `McpScaffoldDraftStore`; the crate must not hard-depend on server-specific DB/ORM implementation.
- `mcp_health` remains an operational introspection tool and must not break from the absence of domain permission mapping.
- `alloy_scaffold_module` may only stage a preview draft crate skeleton and must not:
  - overwrite an existing crate;
  - automatically register the module in the runtime;
  - bypass the review/apply boundary for generated code.
- `alloy_apply_module_scaffold` must require explicit `confirm=true` and must not bypass the preceding review step.
- The persisted scaffold draft control plane lives in `apps/server` (`mcp_scaffold_drafts`, REST `/api/mcp/scaffold-drafts*`, GraphQL `mcpModuleScaffoldDraft*`) and does not replace the local crate API `rustok-mcp`.
- GraphQL and Leptos native adapters must delegate via `McpManagementPort` to the server-owned `McpManagementService`; UI packages and owner GraphQL do not contain scaffold persistence, filesystem apply, or audit SQL.

### Events / Outbox Side Effects
- If the module publishes domain events, publication must go through the transactional outbox/transport contract without local workarounds.
- Event payload and event-type format must remain backward-compatible for cross-module consumers.

### Errors / Failure Codes
- Public `*Error`/`*Result` types of the module define the failure contract and must not lose semantics when mapped to HTTP/GraphQL/CLI.
- For validation/auth/conflict/not-found scenarios, a stable error-class must be maintained, used by tests and adapters.
- For the MCP access layer, the following codes are stable: `tool_disabled`, `tool_not_allowed`, `tool_denied`, `missing_permissions`, `missing_scopes`.
- The runtime tool audit contract via `McpToolCallAuditEvent` recognizes `allowed`/`denied` states but does not override upstream MCP authorization semantics.
- For the scaffold review/apply layer, the following failures are stable: invalid slug/name/description, attempt to directly write during `alloy_scaffold_module`, missing `confirm=true` on `alloy_apply_module_scaffold`, and attempt to write to an already existing target crate.
