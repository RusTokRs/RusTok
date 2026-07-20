# Documentation `rustok-mcp`

`rustok-mcp` is a thin adapter crate for MCP integration in RusToK over `rmcp`.
It holds the RusToK-specific tool/runtime layer, without replacing the official MCP
spec or turning into a provider/model host.

## Purpose

- publish the canonical MCP adapter contract for RusToK;
- keep tool surface, runtime binding, access policy and audit hooks over `rmcp`;
- connect Alloy-related MCP capabilities and the persisted server-side control plane with runtime session flow.

## Responsibilities

- MCP server adapter over `rmcp`;
- typed tools, `McpToolResponse`, runtime binding and access policy contracts;
- session-start access resolution, allow/deny audit and introspection surface;
- Alloy-related MCP tools and scaffold draft review/apply boundary;
- MCP-module-owned admin UI for reviewing Alloy drafts, reading MCP audit and read-side
  clients/policies/token previews: Next package
  `apps/next-admin/packages/rustok-mcp` and Leptos FFA crate `crates/rustok-mcp/admin`;
- typed `McpManagementPort` in the owner crate and DB-provider in `apps/server`, which
  delegates management reads/writes to the canonical transactional `McpManagementService`;
- owner-owned REST/control-plane request and response DTOs for MCP remote transport,
  clients, policy, tokens, audit and scaffold drafts; `apps/server` only maps persisted rows
  and Axum request boundaries to those owner contracts;
- owner-owned GraphQL query/mutation/types behind feature `graphql`; server only connects roots and implements the DB-provider;
- no ownership over provider-specific AI orchestration and over the MCP spec itself.

## Integration

- protocol, security and authorization semantics come from official MCP/rmcp documents, not from the local docs folder;
- `rustok-ai` uses `rustok-mcp` as the MCP tool boundary, without extending it to a model host;
- `apps/server` holds the persisted MCP management/control plane and runtime bridges for tokens, policy and scaffold drafts;
- HTTP handlers in `apps/server` import MCP DTOs and actor parsing from `rustok-mcp`
  instead of defining package-local REST contracts;
- Alloy connects as a capability via runtime state, not as a separate MCP transport stack.
- Alloy script tools use the owner-defined canonical workspace representation.
  Update, delete, and manual-run commands carry `expected_version`;
  execution is pinned to the loaded immutable revision and deletion delegates
  to owner storage CAS.
- `rustok-ai` does not own the UI review of MCP/Alloy drafts; the cross-module admin workflow separately
  mounts the MCP-module-owned package.

## Verification

- structural verification for local docs and the RusToK-specific MCP boundary;
- targeted compile/tests when changing tool surface, access policy, runtime binding or audit path;
- when changing protocol/security assumptions, verification against official MCP/rmcp sources is mandatory.

## External sources of truth

- [MCP docs](https://modelcontextprotocol.io/docs)
- [MCP specification](https://modelcontextprotocol.io/specification/2025-03-26)
- [`rmcp` docs](https://docs.rs/rmcp/latest/rmcp/)
- [Rust SDK repository](https://github.com/modelcontextprotocol/rust-sdk)
- [Authorization guide](https://modelcontextprotocol.io/docs/tutorials/security/authorization)
- [Security Best Practices](https://modelcontextprotocol.io/docs/tutorials/security/security_best_practices)

## Related documents

- [Implementation plan](./implementation-plan.md)
- [Central MCP reference index](../../../docs/references/mcp/README.md)
- [README crate](../README.md)
