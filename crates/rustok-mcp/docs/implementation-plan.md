# rustok-mcp implementation plan

## Current state

`rustok-mcp` owns the RusToK MCP adapter over `rmcp`: typed tools, access
policy, runtime binding, audit hooks, Alloy scaffold review/apply, MCP
management contracts, GraphQL, REST DTOs, and owner-owned Next/Leptos admin
surfaces. `apps/server` supplies persistence, authentication/RBAC extraction,
and composition; it must not recreate MCP DTOs or workflows. The current
protocol surface includes stdio plus authenticated HTTP JSON/SSE transport.
The owner crate also exposes one transport-neutral, read-only registry-tool
invoker. The server uses it for both remote transport and the admitted-artifact
`platform.mcp` route, so tool authorization is not reimplemented by either
host path.

The authenticated remote MCP transport also composes the remote-only
`alloy_import_published_release` tool. It derives tenant and actor identity
from the persisted runtime binding, requires `scripts.manage` and
`modules.manage`, constructs a tenant-scoped Alloy registry, and injects the
same owner-backed published-Rhai source provider as HTTP and GraphQL. Source
bytes are never returned; generic stdio MCP does not advertise this operation.

Generic stdio and in-process MCP expose Alloy scaffold assistance only. They do
not expose tenant-owned Alloy script reads, CRUD, validation, or execution,
because the generic adapter cannot construct the owner-scoped Alloy runtime
that binds a tenant and actor. Canonical script authoring remains on
host-composed Alloy HTTP and GraphQL.

## FFA/FBA status

- FFA status: `in_progress`.
- FBA status: `boundary_ready` (`core_transport_ui`).
- Structural shape: `core_transport_ui`
- `McpManagementPort` is the owner contract. The Leptos package uses
  `HostRuntimeContext` for native `#[server]` functions while retaining the
  parallel GraphQL/headless adapter. Next and Leptos hosts mount owner packages
  without owning MCP management logic.
- Evidence: `scripts/verify/verify-mcp-admin-boundary.mjs`,
  `scripts/verify/verify-api-surface-contract.mjs`, and ADR
  [MCP management owner boundary](../../../DECISIONS/2026-07-10-mcp-management-owner-boundary.md).

## Next results

1. **Obtain authenticated browser parity evidence.** Exercise client, policy,
   token rotation/revocation, audit, and staged Alloy draft review/apply in both
   `/dashboard/mcp` and `/mcp`. Done when native and GraphQL paths have the
   same authorization, transaction-claim, recovery, and visible audit result.
2. **Deliver secure remote MCP transport deliberately.** Define the remote
   session bootstrap, consent, authorization, token storage, HTTPS/TLS,
   redirect validation, SSRF-safe discovery, audit/telemetry, and failure
   policy before promoting beyond stdio. Done when a remote integration test
   proves these controls without token passthrough.
3. **Stage new MCP capabilities by owner contract.** Add resources, prompts,
   roots, sampling, logging, completions, or subscriptions only after a named
   product consumer, permission/policy model, audit semantics, and rollout
   evidence exist. Done when no capability bypasses the management boundary or
   becomes an AI-provider responsibility.
4. **Preserve the artifact capability boundary through worker isolation.**
   The production host maps only the stable `rustok` alias to the owner-defined
   registry tool surface, derives a service identity from the exact admitted
   artifact, applies `McpAccessContext`, and requires durable redacted audit
   before invocation. An isolated sandbox worker must return capability calls
   to this host adapter; it must never receive an MCP endpoint, token,
   credential, database handle, or network client.
5. **Compose remote Alloy authoring through the owner runtime.** Add script
   reads, mutations, validation, or execution to remote MCP only after the
   adapter reuses the same owner-scoped Alloy runtime as host HTTP and GraphQL,
   verifies tenant match and `scripts.manage`, derives actor provenance, applies
   source-redaction policy, and proves tenant isolation in an integration test.

## Verification

- Contract tests cover every public use case.
- `npm run verify:mcp:admin-boundary`
- `node scripts/verify/verify-api-surface-contract.mjs`
- `node scripts/verify/verify-axum-runtime.mjs`
- `cargo check -p rustok-mcp-admin --features ssr`
- `cargo test -p rustok-mcp --lib`

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [MCP reference index](../../../docs/references/mcp/README.md)
- [MCP management owner ADR](../../../DECISIONS/2026-07-10-mcp-management-owner-boundary.md)
