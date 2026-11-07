# ADR: `rustok-ai` as a separate capability module

- Date: 2026-04-03
- Status: Accepted

## Context

RusToK already has `rustok-mcp` as a thin MCP adapter/server surface on top of the official SDK
`rmcp`. At the same time, the product needs a full AI host/orchestrator layer:

- connection to local and cloud model providers;
- orchestration of chat runs;
- calling MCP tools;
- persisted control plane for provider profiles, traces and approvals;
- UI for operator work.

If this layer is embedded into `rustok-mcp`, the MCP server boundary will mix with provider/runtime
orchestration, and `rustok-mcp` will cease to be a thin adapter.

## Decision

Create a separate capability crate `crates/modules/rustok-ai`.

`rustok-ai`:

- owns the Rig 0.46 provider registry, `ProviderSlug`, and provider feature contract;
- owns `RigAgentDriver`, chat/session model and approval policy;
- persists only provider settings and external `SecretRef` values; resolver endpoints,
  cloud identities, allowlists, and egress policy are server-owned composition inputs;
- uses `rustok-mcp` as an MCP tool surface;
- provides `apps/server` with server-side `AiManagementService` and persisted control-plane wiring;
- owns GraphQL query/mutation/subscription roots, DTO and permission checks;
- accepts host-specific RBAC role lookup via `AiGraphqlRoleSlugProvider`, without importing
  server models/services;
- ships a separate Leptos admin UI package `crates/modules/rustok-ai/admin`;
- ships a separate Next.js admin UI package `apps/next-admin/packages/rustok-ai`.

`rustok-mcp` meanwhile remains:

- the MCP transport/protocol boundary;
- the identity/policy/runtime binding layer;
- the tool surface for RusToK and Alloy;
- without provider-specific responsibilities.

## Reasons

### 1. MCP SDK reuse instead of a custom MCP library

RusToK must not maintain its own protocol stack for MCP. The protocol and SDK already live
upstream (`modelcontextprotocol` / `rmcp`), and local code should only implement the
integration layer.

### 2. Provider abstraction must not live in `rustok-mcp`

The `LLM provider <-> host` link is not a responsibility of the MCP server layer. This layer should live
in the AI host/orchestrator capability and use MCP as a separate tool bus.

### 3. Persisted control plane belongs to the capability boundary

Provider profiles, chat sessions, traces, and approvals must be stored through capability-owned
contracts, not in UI hosts and not in `rustok-mcp`. Plaintext provider secrets are forbidden:
`rustok-secrets` resolves server-owned references at execution time.

At the same time, the transport contract does not become server-owned: `rustok-ai` owns the AI GraphQL
resolver/DTO surface, while `apps/server` only adds roots to the common schema and registers narrow
adapters to host persistence.

### 4. UI must remain capability-owned, while the host is only a composition root

Leptos UI is shipped as `crates/modules/rustok-ai/admin`, Next.js UI as
`apps/next-admin/packages/rustok-ai`. This preserves the platform rule:

- module/capability-specific business UI does not go into `apps/admin` or `apps/next-admin`;
- host applications only mount the surface and provide shell/navigation/runtime context.

## Consequences

### Positive

- AI host/orchestrator layer is separated from the MCP server boundary;
- Rig provides the canonical provider protocol/runtime implementation instead of local SSE parsers;
- a registry snapshot makes provider factories, features, schemas, and UI fields auditable;
- dual-path contract for Leptos is preserved: native `#[server]` first, GraphQL parallel;
- AI GraphQL artifacts are not duplicated in `apps/server` and are protected by the owner-boundary guard;
- Leptos and Next.js receive parity capability-owned UI surface;
- `rustok-mcp` remains a thin adapter and does not grow into a separate product runtime.

### Negative

- a new capability crate and a separate persisted control plane appear;
- the Next.js package requires manual `package.json` wiring and manual rebuild;
- Rig upgrades are explicit registry-review changes because the exact crate version is pinned.

### Rig 0.46 owner contract

The owner stores operation-specific SDK `DynModel` handles and uses the current
sans-I/O run API. The SDK owns decoding, stream assembly, and completion detection;
the owner rejects failed/filtered replies and unfinished tool calls. Tenant,
policy, approval, argument validation, cancellation, and billing remain owner rules.
Tool names and call IDs survive every transcript projection, including multiple
results and synthetic skipped peer results. SDK argument normalization must not
turn invalid owner input into an executable tool request.

Unknown provider usage is distinct from zero. Chat accounting requires complete
input/output evidence; embedding and rerank counters remain optional. JSON key
order and float round-tripping are explicitly enabled on the owner's existing
`serde_json` dependency after Rig stopped enabling them transitively.

The OpenAI-compatible dialect permits usage-less embedding replies. The vector
adapter uses Rig's frame hook to validate and order explicit response indices
before Rig pairs vectors with request documents. Rig continues to own transport,
decoding, and usage normalization.

The reviewed provider inventory is `contracts/provider-catalog.json` in the
capability crate. An integration absent from the SDK is not advertised or routed;
persisted provider slugs remain readable for inspection and deactivation. The
normalized event fixtures are not proof of provider protocol interoperability;
offline SDK transport tests and deployment-owned live probes are separate evidence.

## What we are not doing

- not turning `rustok-mcp` into an AI host;
- not writing our own MCP library;
- not moving AI business UI into host applications;
- not making `rustok-ai` a tenant-toggled optional module.
