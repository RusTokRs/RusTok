---
id: doc://docs/architecture/api.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# API Architecture

The API style selection policy is described in [routing.md](./routing.md). This document
captures the top-level API surfaces map of RusToK.

## Brief Summary

RusToK uses a hybrid transport layer:

- GraphQL for UI clients
- REST for integrations, webhooks, ops and module-owned HTTP contracts
- `#[server]` functions for internal Leptos data layer
- OpenAPI for machine-readable REST contract
- health/metrics endpoints for observability

## Canonical Endpoints

| Surface | Endpoint | Purpose |
|---|---|---|
| GraphQL | `/api/graphql` | Single point for admin/storefront UI |
| GraphQL WS | `/api/graphql/ws` | Subscriptions transport |
| GraphQL SDL | `/api/graphql/schema.graphql` | Machine-readable GraphQL schema export for reference artifacts |
| REST | `/api/v1/...` | Integrations, webhooks, batch/ops scenarios |
| MCP management/runtime | `/api/mcp/...` | Persisted MCP clients/tokens/policies/audit and remote runtime bootstrap |
| Commerce REST | `/store/...`, `/admin/...` | Compatible ecommerce HTTP flows |
| OpenAPI | `/api/openapi.json`, `/api/openapi.yaml` | REST contract discovery |
| Health | `/health`, `/health/live`, `/health/ready` | Health and readiness |
| Metrics | `/metrics` | Observability and scraping |

## API Surface Ownership

- `apps/server` owns the common API host layer
- platform modules own domain contracts, resolvers, handlers and service layer
- host applications and UI packages must not become canonical owner of API logic
- module-owned HTTP/GraphQL surfaces must align with manifest wiring and local docs

## API Adapter Placement

API adapters follow the backend module layout:

- owner services, ports, GraphQL roots and REST DTO/handlers live in
  `crates/rustok-<module>/src`;
- published OpenAPI/GraphQL/FBA evidence artifacts live in module-local `contracts/`;
- `apps/server` mounts owner-owned routes, composes schema roots and provides runtime state;
- `apps/server` must not become the owner of module DTOs, resolver policy, command
  providers or business rules;
- maintenance/API-adjacent command flows live in module-local `cli/` adapters over
  `rustok-cli-core`, not in HTTP handlers.

Use `rustok-web` for Axum response/error mapping and `rustok-runtime` only for reusable
runtime helper access. Stable request/port contracts stay in `rustok-api`.

## GraphQL Surface

GraphQL remains the canonical UI-facing contract for:

- Leptos hosts
- Next.js hosts
- module-owned UI packages
- mobile/headless hosts, including Flutter admin/frontend clients

GraphQL must collect domain data through module/service layer, not bypass
module ownership via host-specific shortcuts. Auth bootstrap for headless/mobile hosts uses `me.permissions` as a UI-facing RBAC snapshot; server-side enforcement remains mandatory for the mutations/queries themselves.

Public storefront/read GraphQL queries must not turn absence of
`AuthContext` into `SecurityContext::system()`. Anonymous read flow uses
`SecurityContext::public_read()` (`SecurityActorKind::Public`) and must preserve
module-level published/channel-visible filters alongside reads. `SecurityContext::system()`
is permitted only for trusted platform runtime paths: bootstrap, jobs, migrations,
batch/internal providers and `PortActorKind::System`.

## REST Surface

REST remains mandatory for scenarios where an explicit HTTP contract is needed:

- external integrations
- webhooks
- operational endpoints
- compatible ecommerce flows
- module-owned transport routes
- for post-order ecommerce surface the first OMS slice already includes admin refund routes over `payment-collections` (`/admin/payment-collections/{id}/refunds`, `/admin/refunds/{id}/complete`, `/admin/refunds/{id}/cancel`)

REST must not be used as a hidden replacement for GraphQL for UI-only flows.

MCP runtime bootstrap is a platform-owned REST surface: `POST /api/mcp/runtime/bootstrap` accepts MCP Bearer token or `plaintext_token`, requires non-stdio transport, returns persisted runtime binding/effective access context and writes an audit event with correlation id.

## `#[server]` Surface

Leptos `#[server]` functions are an internal host/UI contract, not a replacement
for the public API surface.

Rules:

- `#[server]` functions are used by default inside Leptos hosts and
  module-owned Leptos UI
- server-side native adapters receive host runtime data through
  `rustok_api::HostRuntimeContext`, not through framework-specific application context
- host configuration needed by a native adapter is supplied as the typed
  `rustok_api::HostSettingsSnapshot` handle, not by extracting a framework app context
- GraphQL is preserved in parallel
- external integrations do not depend on `#[server]`

## Neutral Port Primitives

For migrating modules to Fluid Backend Architecture, the new port layer must
use shared primitives from `rustok-api::ports`:

- `PortContext` passes tenant, actor/service identity, claims/roles, channel, locale,
  correlation/causation, trace context, idempotency key and deadline semantics;
- `PortError` and `PortErrorKind` provide a transport-agnostic domain error envelope prior to mapping
  into GraphQL/REST/gRPC;
- `PortCallPolicy` and `PortOperationKind` provide reusable enforcement for read/write/event-replay/best-effort operations without module-specific behavior;
- read ports must check deadline semantics; write and event-replay ports must check idempotency key and deadline before accessing owner storage
  or remote adapter.

These types are not application service layer and must not contain module-specific
business logic.

`rustok-api` owns `Port*`, permission and locale primitives and does not depend on
`rustok-core` in any feature. Runtime RBAC/security policy belongs to core,
which depends on the API contract layer. Runtime-specific adapters are also not part of the neutral contract surface:
outbox Loco wiring belongs to `rustok-outbox::loco` and is enabled by feature
`rustok-outbox/loco-adapter`.

Backend foundation responsibilities are intentionally split so `rustok-api` remains a
contract crate:

- executable host runtime helpers belong in `rustok-runtime`;
- Axum HTTP response/extractor helpers belong in `rustok-web`;
- FBA provider/consumer metadata and topology descriptors belong in `rustok-fba`;
- platform CLI command/provider contracts belong in `rustok-cli-core`.

When implementing module backend code, use the backend module guides as the operational
contract:

- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Backend Module Implementation Guide](../backend/module-backend-implementation.md)
- [Backend Module Verification Guide](../backend/module-backend-verification.md)

New module services, ports, HTTP handlers, GraphQL roots, `#[server]` adapters and CLI
adapters must not use Loco runtime APIs as their target contract. During the current
cutover, any remaining Loco controller/router usage is legacy boundary inventory only.

## Security and Context Contract

Each API path must operate through a single host/runtime context:

- tenant resolution
- request-scoped locale
- auth/session handling
- request-scoped `ChannelContext`, including `resolution_source` and `resolution_trace` for channel-aware runtime diagnostics
- RBAC enforcement
- observability hooks

For the full application router, the canonical request context preparation order:
`security_headers -> tenant::resolve -> locale::resolve_locale -> auth_context::resolve_optional -> channel::resolve -> handler`.
`channel::resolve` must build `RequestFacts` from tenant id, request selectors, effective host,
auth-derived OAuth/client dimension and effective locale; the channel cache key must distinguish locale/OAuth dimensions,
so that one client/locale request does not reuse resolution from a different context.

API surface must not bypass these layers through local shortcuts.

### Request-context/channel Invariant Verification

For changes to middleware, channel resolution, locale/auth extensions or cache key,
a fast source-level gate is mandatory:

```bash
node scripts/verify/verify-runtime-context-invariants.mjs
./scripts/verify/verify-all.sh runtime-context-invariants
```

This verification enforces the following runtime contracts without full Rust compilation:

- `build_request_facts` reads OAuth/client dimension from `AuthContextExtension`;
- `build_request_facts` reads effective locale from `ResolvedRequestLocale`;
- `ChannelCacheKey` contains `oauth_app_id` and `locale`, including negative cache entries;
- source-order middleware in `compose_application_router` preserves the actual Axum execution order
  `locale -> auth_context -> channel`;
- tenant locale cache metrics remain exportable via `/metrics`.

If the verification fails, do not just fix the textual order of `.layer(...)`: restore
the actual behavior of request extensions, cache-key dimensions and
observability evidence.

## Reference Artifacts (DOC-09)

For contract-level API changes, updatable reference artifacts are mandatory:

- OpenAPI snapshots (`/api/openapi.json`, `/api/openapi.yaml`)
- GraphQL snapshots: full introspection (`/api/graphql`) and SDL (`/api/graphql/schema.graphql`)
- rustdoc artifacts for `rustok-server` and `rustok-workflow`

Canonical local export is performed via:

```bash
node scripts/verify/export-reference-artifacts.mjs artifacts/reference
node scripts/verify/verify-reference-artifacts.mjs artifacts/reference
```

The Unix/CI wrapper `scripts/verify/export-reference-artifacts.sh` delegates to the same
Node.js exporter, so Windows and CI produce an identical layout.

Rule: when changing GraphQL/REST/`#[server]` contract, the PR must contain
Verification Evidence for artifact export and a link to the diff.

## API Compatibility

- GraphQL, REST, OpenAPI and `#[server]` contracts are considered public for their
  target clients and must not be removed without a documented migration path.
- Breaking change requires an explicit migration description in the PR and updates to local
  module/app docs.
- A new Leptos `#[server]` path must not replace an existing GraphQL/REST contract,
  if that contract is already used as a fallback or headless surface.
- For revision-aware control-plane mutations, stale client state must receive
  a conflict-style error, not a silent overwrite or blind rollback.

## Tenant Isolation and RLS

- The base model remains shared DB/shared schema with `tenant_id` as a mandatory
  application/runtime boundary.
- DB-level RLS is a target hardening layer for high-risk tenant-scoped
  tables, but is enabled staged: first platform-control/tenant-module pilot
  after request-scoped tenant DB session context becomes available.
- Broad RLS big-bang migration is prohibited without a separate ADR and rollback plan.

## What Not To Do

- do not mix API contract ownership between host and module crate
- do not duplicate transport flows without a clear reason
- do not consider a UI package as the source of truth for API surface
- do not introduce a separate locale/auth contract at a specific endpoint family level

## Related Documents

- [Routing and Transport Layer Boundaries](./routing.md)
- [GraphQL and Leptos Server Functions](../UI/graphql-architecture.md)
- [Backend Module Architecture](../backend/module-backend-architecture.md)
- [Backend Module Implementation Guide](../backend/module-backend-implementation.md)
- [Module Architecture](./modules.md)
- [Platform Architecture Overview](./overview.md)
