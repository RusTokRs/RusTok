# rustok-mcp implementation plan

## Current state

`rustok-mcp` owns the RusToK MCP adapter over `rmcp`: typed tools, access
policy, runtime binding, audit hooks, Alloy scaffold review/apply, MCP
management contracts, GraphQL, REST DTOs, and owner-owned Next/Leptos admin
surfaces. `apps/server` supplies persistence, authentication/RBAC extraction,
and composition; it must not recreate MCP DTOs or workflows. The current
protocol surface is stdio only.

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
