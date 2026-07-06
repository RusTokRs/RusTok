---
id: doc://docs/references/mcp/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# MCP Reference Package

Last updated: **2026-03-20**.

This package does not duplicate the MCP specification and does not retell the `rmcp` documentation. Its goal:

- to give the team a short index of official sources of truth;
- to capture exactly what RusToK uses from MCP today;
- to protect local documents from degrading into an outdated retelling of a rapidly developing ecosystem.

## Sources of truth

### Official documentation and specification

- MCP docs: [modelcontextprotocol.io/docs](https://modelcontextprotocol.io/docs)
- MCP spec: [modelcontextprotocol.io/specification](https://modelcontextprotocol.io/specification/2025-03-26)
- Server tools: [Tools](https://modelcontextprotocol.io/specification/2025-03-26/server/tools)
- Server resources: [Resources](https://modelcontextprotocol.io/specification/2025-03-26/server/resources)
- Server prompts: [Prompts](https://modelcontextprotocol.io/specification/2025-03-26/server/prompts)

### Security and authorization

- Authorization guide: [Understanding Authorization in MCP](https://modelcontextprotocol.io/docs/tutorials/security/authorization)
- Security guide: [Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

### Rust SDK

- `rmcp` docs: [docs.rs/rmcp](https://docs.rs/rmcp/latest/rmcp/)
- Official Rust SDK repo: [modelcontextprotocol/rust-sdk](https://github.com/modelcontextprotocol/rust-sdk)

## How to use this package

If the question concerns:

- protocol structure;
- MCP capability surface;
- server/client semantics;
- authorization flow;
- security requirements;
- specific `rmcp` behavior;

you should go to the official links above. Local RusToK documents should reference them, not
copy specification fragments into themselves.

## What we document locally in RusToK

Locally we document only the integration layer:

- `rustok-mcp` as a thin adapter over `rmcp`;
- which tool surface is already implemented;
- which RusToK-specific constraints and gaps remain;
- how MCP relates to Alloy and platform RBAC/tenant model.

## Current state of RusToK

As of today `rustok-mcp` covers:

- MCP server/tool surface via `rmcp`;
- module discovery tools;
- Alloy-related tools when `AlloyMcpState` is present;
- identity/policy foundation via `McpIdentity`, `McpAccessContext`, `McpAccessPolicy`;
- introspection tool `mcp_whoami`;
- compatibility shim via legacy `enabled_tools`;
- session-start runtime binding hooks (`McpSessionContext`, `McpAccessResolver`, `McpRuntimeBinding`);
- runtime allow/deny audit hook via `McpAuditSink`;
- the first real Alloy product-slice: `alloy_scaffold_module`, which stages a draft `crates/rustok-<slug>` module scaffold, while `alloy_review_module_scaffold` / `alloy_apply_module_scaffold` provide the review/apply boundary.
- persisted server-side control plane for Alloy scaffold drafts in `apps/server` via REST `/api/mcp/scaffold-drafts*` and GraphQL `mcpModuleScaffoldDraft*`.
- live runtime hook `McpScaffoldDraftStore`, through which `DbBackedMcpRuntimeBridge` can move the Alloy scaffold flow from process-local memory into persisted drafts in `apps/server`.

As of today `rustok-mcp` does not cover as a production-ready layer:

- server-owned remote MCP transport/session bootstrap beyond the current stdio adapter path;
- admin UI for MCP clients, tokens, policies and audit;
- full upstream capability surface (`resources`, `prompts`, `roots`, `sampling`, etc.);
- product UI and server-owned remote MCP bootstrap on top of the already existing persisted scaffold draft control plane;
- a richer review/apply/codegen pipeline that turns a draft Alloy scaffold into a production-ready module.

The persisted management layer already exists on the `apps/server` side: tables `mcp_clients`, `mcp_tokens`,
`mcp_policies`, `mcp_audit_logs`, `mcp_scaffold_drafts`, REST `/api/mcp/*`, GraphQL `mcp*` and
`DbBackedMcpRuntimeBridge` for binding a plaintext MCP token to runtime access context and runtime
allow/deny audit.

## Related local documents

- [`crates/rustok-mcp/README.md`](../../../crates/rustok-mcp/README.md)
- [`crates/rustok-mcp/docs/README.md`](../../../crates/rustok-mcp/docs/README.md)
- [`crates/rustok-mcp/docs/implementation-plan.md`](../../../crates/rustok-mcp/docs/implementation-plan.md)
- [`docs/modules/registry.md`](../../modules/registry.md)
- [`docs/modules/crates-registry.md`](../../modules/crates-registry.md)

## Maintenance rule

Before any update of local MCP docs:

1. Check the current official MCP/rmcp documents.
2. Do not move long retellings of spec/SDK into RusToK.
3. Document only local integration, constraints and RusToK decisions.
