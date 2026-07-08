# Implementation plan for `rustok-mcp`

Status: governed MCP tool adapter already works on top of `rmcp`; the next work
is not about rewriting the protocol, but about bringing RusToK-specific runtime,
identity/audit and Alloy-related control plane to platform-grade level.

## Execution checkpoint

- Current phase: `mcp_admin_native_loco_free_transport`; previous owner UI checkpoints: `mcp_admin_owner_ui_slice`, `mcp_leptos_host_composition`.
- Last checkpoint: MCP GraphQL query/mutation/types and MCP REST/control-plane request/response DTOs moved from `apps/server` to `rustok-mcp` and work through a unified owner-defined `McpManagementPort`; server provider delegates reads/writes to canonical `McpManagementService`. Leptos native `#[server]` functions now read DB and `ModuleRuntimeExtensions` through `HostRuntimeContext` typed host handles, and `rustok-mcp-admin` no longer depends on `loco-rs`, so GraphQL, FFA and HTTP adapters maintain unified transaction claim/recovery semantics without host-owned resolver/DTO or package-local Loco context.
- Next step: add authenticated browser-level parity smoke for Next `/dashboard/mcp` and Leptos `/mcp` management workflows over the already strengthened draft stage/apply boundary.
- Open blockers: no active local blockers for the current MCP slice; `cargo check -p rustok-mcp-admin --features ssr`, `npm run verify:mcp:admin-boundary`, `node scripts/verify/verify-api-surface-contract.mjs` and `node scripts/verify/verify-loco-inventory.mjs` pass for the current native transport cutover.
- Hand-off notes for next agent: keep `rustok-mcp` as MCP protocol/tool adapter, leave persisted draft storage in `apps/server`, and UI in owner surface MCP, not in `rustok-ai`. After tool surface changes, repeat `cargo check -p rustok-mcp-admin --features ssr`, `npm run verify:mcp:admin-boundary`, `cargo check -p rustok-server`, `cargo check -p rustok-mcp` and `cargo test -p rustok-mcp --lib`.
- Last updated at (UTC): 2026-07-08T00:00:00Z

## FFA/FBA status

- Structural shape: `core_transport_ui`

- FFA status: `in_progress`
- FBA status: `in_progress`
- Evidence:
  - Next owner surface `apps/next-admin/packages/rustok-mcp` owns UI review of MCP/Alloy scaffold drafts, audit events, clients/policies/tokens and management mutations; host route only mounts `McpAdminPage`.
  - Leptos FFA surface `crates/rustok-mcp/admin` contains Leptos-free `core.rs`, `transport::{native_server_adapter,graphql_adapter}` and explicit `ui/leptos.rs` adapter.
  - Leptos host `apps/admin` connects the owner crate through a thin `/mcp` route; CSR, hydrate WASM and SSR feature profiles compile.
  - Native `#[server]` functions are the monolith/hydrate selected path for Leptos; they consume `HostRuntimeContext` for DB and typed `ModuleRuntimeExtensions` host handles without package-local Loco context, mutations and owner GraphQL receive `McpManagementRuntime` from `ModuleRuntimeExtensions`, and the server provider delegates client/policy/token/audit/scaffold reads/writes to `McpManagementService`. GraphQL operation documents remain in `transport/graphql_adapter.rs` in parallel.
  - MCP REST/control-plane request and response DTOs for remote transport, clients, policy, tokens, audit and scaffold drafts live in `crates/rustok-mcp/src/management.rs`; `apps/server/src/controllers/mcp.rs` imports those owner types and only maps persisted rows plus Axum/Loco boundaries.
  - MCP actor type parsing lives in `rustok-mcp` through `McpActorType::from_str`; the server controller delegates to that owner contract instead of duplicating string taxonomy.
  - Boundary guardrail `scripts/verify/verify-mcp-admin-boundary.mjs` checks owner placement, requires HostRuntimeContext usage, requires stage/apply delegation through mutation port, prohibits scaffold persistence/audit SQL in UI adapter and prohibits MCP draft UI inside `rustok-ai`.
- Last verified at (UTC): 2026-07-08T00:00:00Z.
- Owner: `rustok-mcp`.

## Scope of work

- keep `rustok-mcp` as a thin MCP adapter crate on top of `rmcp`;
- synchronize tool surface, runtime binding, access policy and local docs;
- prevent mixing MCP protocol boundary with AI provider orchestration.

## Current state

- crate is already integrated with `rmcp` and ships as library + binary;
- module discovery tools, health/introspection, Alloy-related tools and scaffold review/apply boundary are already in place;
- persisted server-side scaffold drafts and runtime draft-store bridge are already connected to MCP flow;
- identity/policy foundation, session-start runtime binding and allow/deny audit are already part of the live contract.

## Stages

### 1. Contract stability

- [x] lock `rustok-mcp` as a thin adapter on top of `rmcp`;
- [x] bring up typed tool surface, response envelope and access-policy baseline;
- [x] embed Alloy-related scaffold/review/apply vertical and runtime draft-store binding;
- [ ] maintain sync between runtime contracts, management/control plane and local docs.

### 2. Platform hardening

- [ ] bring server-owned remote MCP transport/session bootstrap beyond the current stdio path;
- [ ] expand audit trail from allow/deny to richer execution telemetry;
- [ ] keep identity/policy layer compatible with official MCP authorization guidance.

### 3. Product surface

- [ ] add UI layer for MCP access management and Alloy draft review;
- [ ] expand Alloy/codegen vertical without automatic blurring of review/apply boundary;
- [ ] add new MCP capabilities (`resources`, `prompts`, `sampling`, etc.) only as explicit staged rollout.

## Verification

- structural verification for RusToK-specific MCP docs and boundary;
- targeted compile/tests when tool surface, access policy, runtime binding or draft-store integration changes;
- mandatory cross-check with official MCP/rmcp docs when changing protocol/security assumptions.

- contract tests cover all public use-case MCP surface.

## Update rules

1. When changing RusToK-specific MCP contract, update this file first.
2. First cross-check changes with official MCP/rmcp sources, then update local docs.
3. When changing public crate behavior, synchronize `README.md` and `docs/README.md`.
4. When changing reference-map, update `docs/references/mcp/README.md` and if necessary `docs/index.md`.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
