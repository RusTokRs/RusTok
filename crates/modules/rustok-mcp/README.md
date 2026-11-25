# rustok-mcp

## Purpose

`rustok-mcp` is RusToK's MCP capability owner. It integrates platform services with the official Rust
SDK (`rmcp`) and keeps MCP transport/protocol concerns out of business-domain crates.

## Responsibilities

- Expose RusToK functionality as typed MCP tools.
- Keep MCP protocol and transport concerns out of domain crates.
- Provide the governed bridge between MCP clients and platform capabilities such as Alloy.
- Preserve the boundary between protocol integration and business/runtime ownership.

## Scope

- Keep MCP support as a thin adapter layer over `rmcp`.
- Expose RusToK functionality as typed MCP tools with generated JSON Schemas.
- Return tool payloads in a stable response envelope (`McpToolResponse`).
- Provide a governed AI-to-platform boundary for Alloy and other platform capabilities.
- Avoid re-implementing MCP protocol details inside RusToK documentation or code comments.

## Upstream source of truth

The MCP protocol and the Rust SDK evolve independently from RusToK. Local docs in this crate
describe only the RusToK integration layer and current implementation gaps.

Authoritative upstream references:

- MCP documentation: [modelcontextprotocol.io/docs](https://modelcontextprotocol.io/docs)
- MCP specification: [modelcontextprotocol.io/specification](https://modelcontextprotocol.io/specification/2025-03-26)
- Official Rust SDK docs (`rmcp`): [docs.rs/rmcp](https://docs.rs/rmcp/latest/rmcp/)
- Official Rust SDK repository: [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)
- MCP authorization guidance: [Understanding Authorization in MCP](https://modelcontextprotocol.io/docs/tutorials/security/authorization)
- MCP security guidance: [Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

If there is any mismatch between local RusToK docs and the official MCP/rmcp docs, treat the
official docs as the source of truth for protocol/SDK behavior.

## Current RusToK implementation

### Module tools

- `list_modules`
- `query_modules`
- `module_exists`
- `module_details`
- `content_module`
- `blog_module`
- `forum_module`
- `pages_module`

### MCP control tools

- `mcp_health`
- `mcp_whoami`

### Alloy scaffold tools

Available when `AlloyScaffoldState` is configured:

- `alloy_import_published_release` is available only through the authenticated
  server remote MCP JSON/SSE transport. It derives tenant and actor identity
  from the durable MCP runtime binding, requires `scripts.manage` plus
  `modules.manage`, and imports an exact owner-projected Rhai release without
  returning source bytes. Generic stdio MCP does not advertise this tool.
- `alloy_scaffold_module`
- `alloy_review_module_scaffold`
- `alloy_apply_module_scaffold`
- `alloy_list_entity_types`
- `alloy_script_helpers`

Generic stdio and in-process MCP do not expose tenant-owned Alloy script reads,
CRUD, validation, execution, or deleted-evidence retention authority. The
generic adapter has no owner-scoped Alloy runtime, so exposing those operations
would make tenant binding and actor audit ambiguous. Authenticated server remote
JSON/SSE transport composes the remote-only `alloy_*_script` authoring and
`alloy_*_deleted_evidence_retention` tools from the same tenant-scoped Alloy
runtime as HTTP and GraphQL. It requires `scripts.manage`, derives actor and
tenant from the durable MCP binding, checks identity-to-binding tenant equality,
and returns source-redacted or source-free results. Those names are guarded from
generic MCP.

`alloy_scaffold_module` stages a reviewed design scaffold for
`crates/modules/rustok-<slug>`; it records requested transport surfaces in documentation
but never creates fake GraphQL or REST handlers. The actual workspace write is
separated into `alloy_apply_module_scaffold` with explicit confirmation. Stage, review, and
apply responses include an owner-issued canonical `sha256:` source digest for the logical crate
root and generated files; this is provenance evidence, not a hash inferred from MCP text.

### What is implemented today

- `stdio` MCP server bootstrapped through `rmcp`
- typed tool schemas and a stable `McpToolResponse`
- config-level tool allow-list through `enabled_tools`
- explicit MCP identity + tool policy foundation (`McpIdentity`, `McpAccessContext`, `McpAccessPolicy`)
- permission-aware tool authorization with compatibility shim for legacy `enabled_tools`
- `mcp_whoami` for identity/permissions/scopes/policy introspection
- runtime auth hooks for persisted sessions (`McpSessionContext`, `McpAccessResolver`, `McpRuntimeBinding`, `McpAuditSink`)
- pluggable scaffold draft runtime hooks (`McpScaffoldDraftRuntimeContext`, `McpScaffoldDraftStore`)
- runtime allow/deny audit events for tool invocations
- module-owned persisted MCP management for clients/tokens/policies/audit
  GraphQL: `mcpClients`, `mcpClient`, `mcpAuditEvents`, `createMcpClient`, `rotateMcpClientToken`, `updateMcpClientPolicy`, `revokeMcpToken`, `deactivateMcpClient`
  REST: `/api/mcp/*`
- embedded `McpPersistence` resolves persisted tokens, records audit and implements the management and draft-store ports
- optional Alloy tool surface layered on top of `alloy`
- staged RusToK module scaffolding through `alloy_scaffold_module`
- explicit review/apply boundary for generated drafts through `alloy_review_module_scaffold` and `alloy_apply_module_scaffold`
- persisted Alloy scaffold draft control plane through REST `/api/mcp/scaffold-drafts*` and GraphQL `mcpModuleScaffoldDraft*`
- transaction-claimed persisted scaffold apply flow: applying a draft first
  atomically claims `staged -> applying` and writes an `apply_started` audit row, then records either
  the final `applied` state with success audit, restores `staged` on a confirmed pre-publication failure, or records `recovery_required` when publication is uncertain
- live runtime binding hooks so Alloy scaffold tools can use the persisted draft store instead of process-local memory when a server-backed `McpScaffoldDraftStore` is attached
- owner-owned admin UI slice for persisted Alloy scaffold drafts, MCP audit events, clients,
  policies, and safe token previews:
  - Next package `apps/next-admin/packages/rustok-mcp`
  - Leptos FFA package `crates/modules/rustok-mcp/admin`
  - Next host route `/dashboard/mcp` and Leptos host route `/mcp` only mount the owner packages and
    do not own MCP transport logic
- REST, GraphQL and native admin use the same `McpManagementPort`. The host supplies composition dependencies.
- owner-defined GraphQL query, mutation, and DTO surface behind the `graphql` feature

### What is not implemented yet

- MCP `resources`, `prompts`, `roots`, `sampling`, `logging`, `completions`, or subscriptions
- standard MCP Streamable HTTP and independent owner deployment. The embedded host currently exposes RusToK JSON/SSE routes.
- browser-level parity verification for the Next and Leptos management workflows
- script-to-native-module compilation pipeline and marketplace/publish flow
- automatic generation of module-specific `Resource`/`Permission` surface in `rustok-core`

## Quick start

```rust
use rustok_core::registry::ModuleRegistry;
use rustok_mcp::McpServerConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let registry = ModuleRegistry::new();
    let config = McpServerConfig::new(registry);
    rustok_mcp::serve_stdio(config).await
}
```

To enable a tool allow-list:

```rust
use std::collections::HashSet;

use rustok_core::registry::ModuleRegistry;
use rustok_mcp::McpServerConfig;

let registry = ModuleRegistry::new();
let enabled = HashSet::from([
    "list_modules".to_string(),
    "mcp_health".to_string(),
]);
let config = McpServerConfig::with_enabled_tools(registry, enabled);
```

To enable Alloy scaffold tools, construct the server with `with_scaffold_tools`:

```rust
use rustok_core::registry::ModuleRegistry;
use rustok_mcp::{AlloyScaffoldState, RusToKMcpServer};

let registry = ModuleRegistry::new();
let scaffolds = AlloyScaffoldState::new();
let server = RusToKMcpServer::with_scaffold_tools(registry, scaffolds);
```

To attach an MCP identity and permission-aware tool policy:

```rust
use rustok_api::Permission;
use rustok_mcp::{McpAccessContext, McpAccessPolicy, McpActorType, McpIdentity, McpServerConfig};

let config = McpServerConfig::new(registry).with_access_context(McpAccessContext {
    identity: Some(McpIdentity {
        actor_id: "model:writer".to_string(),
        actor_type: McpActorType::ModelAgent,
        tenant_id: Some("tenant-1".to_string()),
        delegated_user_id: Some("user-42".to_string()),
        display_name: Some("Writer agent".to_string()),
        scopes: vec!["modules.read".to_string()],
    }),
    granted_permissions: vec![
        Permission::MODULES_READ.to_string(),
        Permission::MODULES_LIST.to_string(),
    ],
    policy: McpAccessPolicy {
        allowed_tools: Some(vec![
            "list_modules".to_string(),
            "module_details".to_string(),
            "mcp_whoami".to_string(),
        ]),
        denied_tools: Vec::new(),
    },
});
```

For persisted MCP auth, compose the owner with the `persistence` feature.
`McpPersistence` implements `McpManagementPort`, `McpAccessResolver`,
`McpAuditSink` and `McpScaffoldDraftStore`. The host supplies the database,
effective principal and permission facts, and the authorized workspace
publisher. The `migrations` feature exports the owner migration source with
the existing schema names and ordering.


## Interactions

- embedded binary target `rustok-mcp-server`
- `crates/libs/rustok-core` for registry/services
- domain and capability crates through their service layers
- Alloy scaffold contracts when scaffold tools are enabled
- `apps/server` for embedded composition, HTTP routing and host workspace publication

## Entry points

- `serve_stdio`
- `McpServerConfig`
- `RusToKMcpServer`
- `AlloyScaffoldState`
- MCP tool registry exported from `src/lib.rs`

## Docs

- Public API contract: [CRATE_API.md](./CRATE_API.md)
- [Module docs](./docs/README.md)
- RusToK MCP implementation plan: [`./docs/implementation-plan.md`](./docs/implementation-plan.md)
- Central MCP reference index: [`../../docs/references/mcp/README.md`](../../../docs/references/mcp/README.md)
- Platform docs map: [`../../docs/index.md`](../../../docs/index.md)
