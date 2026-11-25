# Documentation `rustok-mcp`

`rustok-mcp` owns MCP integration and persisted MCP management in RusToK. The protocol adapter uses `rmcp`.
It holds the RusToK-specific tool/runtime layer, without replacing the official MCP
spec or turning into a provider/model host.

## Purpose

- publish the canonical MCP adapter contract for RusToK;
- keep tool surface, runtime binding, access policy and audit hooks over `rmcp`;
- connect Alloy-related MCP capabilities and the persisted MCP control plane with runtime session flow.

## Responsibilities

- MCP server adapter over `rmcp`;
- typed tools, `McpToolResponse`, runtime binding and access policy contracts;
- session-start access resolution, allow/deny audit and introspection surface;
- transport-neutral invocation of the owner-defined read-only registry tools,
  reused by authenticated remote transport and admitted artifact capability
  composition;
- Alloy-related MCP tools and scaffold draft review/apply boundary;
- MCP-module-owned admin UI for reviewing Alloy drafts, reading MCP audit and read-side
  clients/policies/token previews: Next package
  `apps/next-admin/packages/rustok-mcp` and Leptos FFA crate `crates/modules/rustok-mcp/admin`;
- typed `McpManagementPort` and the embedded persistence provider for clients, policies, tokens, audit and scaffold drafts;
- module-owned SeaORM entities and schema migrations with stable published names;
- owner-owned REST/control-plane request and response DTOs;
- owner-owned GraphQL query/mutation/types behind feature `graphql`; the server connects the roots and registers the embedded owner;
- no ownership over provider-specific AI orchestration and over the MCP spec itself.

## Integration

- protocol, security and authorization semantics come from official MCP/rmcp documents, not from the local docs folder;
- `rustok-ai` uses `rustok-mcp` as the MCP tool boundary, without extending it to a model host;
- `apps/server` supplies the database, authenticated request context, delegated principal facts, effective module availability and authorized workspace publication;
- HTTP handlers in `apps/server` import MCP DTOs and actor parsing from `rustok-mcp`
  instead of defining package-local REST contracts;
- Alloy connects as a capability via runtime state, not as a separate MCP transport stack.
- Generic MCP owns only Alloy scaffold assistance. It deliberately does not
  expose tenant-owned Alloy script reads, CRUD, validation, or execution,
  because it cannot construct the owner-scoped Alloy runtime required for
  tenant binding and actor provenance. Script authoring remains on the
  host-composed Alloy HTTP and GraphQL surfaces and on the authenticated host-composed JSON/SSE tool surface.
- admitted artifacts can call only the host-configured `rustok` server alias.
  The server derives their service identity from exact installation scope,
  applies the MCP owner policy, persists redacted audit evidence before the
  call, and returns only the JSON tool envelope. Artifact input cannot select
  transport, endpoint, token, credentials, or discovery.
- Alloy script tools use the owner-defined canonical workspace representation.
  Update, delete, and manual-run commands carry `expected_version`;
  execution is pinned to the loaded immutable revision and deletion delegates
  to owner storage CAS.
- The remote-only `alloy_import_published_release` tool derives tenant and actor
  from the persisted MCP binding, requires `scripts.manage` and
  `modules.manage`, and invokes the owner-backed published-Rhai source provider
  through a tenant-scoped Alloy registry. It returns redacted draft identity
  and parent lineage only; generic stdio MCP cannot advertise the tool.
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
- [Central MCP reference index](../../../../docs/references/mcp/README.md)
- [README crate](../README.md)
