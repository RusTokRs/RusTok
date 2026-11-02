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

Product workflow write semantics and the incomplete editor aggregate-save remediation are tracked
by the [Product owner plan](../../crates/modules/rustok-product/docs/implementation-plan.md#workflow-remediation-2026-10-17).
Owner document updates validate Active requirements against their candidate state before commit;
create/update compose typed locale/value batches inside that same transaction, not through a
second independently receipted schema command. Next status actions use lifecycle-only
updates rather than a partial editor-document save. All fourteen catalog commands now bind caller keys to durable owner receipts with pre-commit
public results. Atomic document/category/Product EAV/publication save is source-implemented in Next
and Leptos; database execution, whole-screen Variant atomicity and child-command recovery remain
open. Next save/status/delete retain immutable requests in a scope/resource/URL-intent browser
journal, with exclusive Web Locks, explicit recovery and terminal tombstones; live browser/backend
reload/concurrency evidence remains open. GraphQL `outcome: rejected` distinguishes a finalized
domain refusal from admission/context failures, even when both are nonretryable. The Product [write boundary](../../crates/modules/rustok-product/docs/write-boundary.md)
records lifecycle receipt semantics separately from the receipt-bound EAV surface and Proposed Media workflow.
Product/Pricing money uses decimal strings in major units at Next/GraphQL boundaries. Catalog
Variant price replacement is limited to unscoped base rows; contextual prices use Pricing commands
and survive catalog edits. See [base-price semantics](../../crates/modules/rustok-product/docs/write-boundary.md#catalog-base-price-replacement).

## API Adapter Placement

API adapters follow the backend module layout:

- owner services, ports, GraphQL roots and REST DTO/handlers live in
  `crates/modules/rustok-<module>/src`;
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
which depends on the API contract layer. Runtime-specific adapters are also not
part of the neutral contract surface: the server composes the host-neutral
outbox runtime through explicit `ServerRuntimeContext` values.

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
adapters must use explicit owner and Axum runtime contracts as their target
contract. The active server and generated module composition are Axum-only.

## HTTP error retry advice

The shared Axum error-envelope owner is [`rustok-web`](../../crates/libs/rustok-web/CRATE_API.md),
not a module-local Pricing/Commerce envelope. Its flat `ErrorBody` retains `code` and `message`
and adds optional `retryable`. The same DTO derives the OpenAPI schema.

```json
{
  "code": "pricing.product_identity_capability_unavailable",
  "message": "The requested service is temporarily unavailable",
  "retryable": false
}
```

- A present boolean is exact owner advice. Both false and true survive status/body/JSON mapping;
  the shared port mapper forwards `PortError.retryable`, and known custom adapters forward their
  existing owner/final-runtime decision. Status/code/public-copy/redaction policies do not change.
- Missing is valid unknown advice, not false or true. Ordinary HTTP constructors with no owner
  decision omit the member. A present null, string, number, array or object is malformed; the
  canonical DTO rejects it and its schema is optional but non-nullable. No status or public
  message phrase substitutes for a missing bit, or overrides an explicit false on 503/504.
- `apps/server::Error::Http` delegates the same response. The existing Next Admin registry REST
  helper uses the host's shared HTTP decoder: owner/absent/invalid evidence stays distinguishable,
  malformed advice stays unknown, and unsupported raw/nested bodies never become public text.
  Its metadata reader admits 16 KiB within two seconds, releases/cancels its reader, and does not
  await failed or stalled upstream cancellation; runtime timing/cancellation remains unverified.
- Advice is **not durable outcome**. It does not establish commit/rejection, release retained
  attempts, permit fallback/automatic write replay, replace a frozen request/idempotency key,
  bypass current authorization, or supersede owner receipts/journals/recovery.
- This is an additive optional public-wire member, not an internal compatibility mode or a second
  error model. Current buyer Product/Pricing/Cart UI calls remain GraphQL/native; no REST buyer
  client is invented. GraphQL extensions and native envelopes are unchanged by this HTTP slice.

The [Commerce source handoff](../../crates/modules/rustok-commerce/docs/implementation-plan.md#canonical-http-retry-advice-2026-10-30)
records actual forwarding/schema/client coverage, source-only checks and unexecuted runtime gates.
Domain error representations in `rustok-core` do not replace the Axum envelope owner.

## Cart priced mutation adoption

Cart owns [`CartPricingContextGuard`](../../crates/modules/rustok-cart/CRATE_API.md#priced-mutation-adoption-2026-10-30-source-continuation)
for the original observation used by an external price/stock read. It is an internal owner
precondition, not a browser-authority token, monotonic Cart revision or durable receipt.

- Priced add/change/reprice bind tenant/Cart, saved currency/region/channel/locale/country,
  customer/capability evidence and exact line identity/quantity/money/fulfillment inputs. They
  compare under the existing Cart row lock before writes. An authorization wrapper may bind
  already-authorized guest evidence but must not refresh material inputs after the quote.
- Priced observations are bounded to 1024 lines; insertion at capacity refuses. Explicit removal
  uses a purpose-bound target-only observation, so larger/invalid-price historical carts can be
  reduced. It cannot authorize price adoption. A decrement-to-removal retains its original
  target quantity; context or target drift refuses instead of deleting a newer line.
- Bulk updates carry quoted quantity and reject duplicate/unknown/stale rows before any write.
  The unquoted storefront quantity operation is retired; absent original SKU identity is not
  permission to scale a stored price or substitute a Product/variant.
- Commerce shares one REST/GraphQL/native quantity workflow: selected Product/publication/
  fulfillment and Pricing checks precede Inventory's aggregate **replacement** demand check.
  Price tiers remain per target line, not aggregate SKU stock demand. Owner errors preserve exact
  retry advice; no hint authorizes automatic replay or a new command identity.
- Native request/cart DTO shapes and endpoints remain unchanged. Native uses guarded ports and
  the existing admitted request context/claims; sanitized guest projections are not secrets.
  Owner failures use one static inspect-before-retry envelope, not raw diagnostic messages.

This source continuation does not provide price/stock holds, cross-owner atomicity, exact replay,
monotonic revisions, live policy-revocation fencing or Cart mutation/outbox receipts. The
2026-10-31 continuation below closes Cart-local context/price atomicity. Its regressions are unrun; the
[Cart plan](../../crates/modules/rustok-cart/docs/implementation-plan.md#priced-quantity-adoption-2026-10-30)
and [Commerce handoff](../../crates/modules/rustok-commerce/docs/implementation-plan.md#cart-quantity-adoption-2026-10-30)
record scoped source checks and remaining compilation/runtime/concurrency/downstream gates.

## Cart context and Pricing adoption (2026-10-31)

The [Cart owner command](../../crates/modules/rustok-cart/CRATE_API.md#atomic-context-and-pricing-adoption-2026-10-31-source-continuation)
is one `CartContextUpdateRequest` with full replacement input, original material observation and
explicit `CartContextPricing`. Browser DTOs do not accept money/plans. Commerce prepares selected
Pricing before Cart writes using owner-normalized hypothetical target, expected parent plus
per-line tier groups, deduplicated/chunked batches, exact response correlation and one inherited
price-operation deadline. A batch refusal is not permission to use single reads, old prices or
another provider.

Cart checks original observation and exact complete target coverage under its lock, then commits
context, prices/adjustments, marketplace projection and existing shipping/tax/totals in one owner
transaction. Preserve is explicit checkout/materialization semantics: nonempty Cart cannot change
currency/region/channel pair/locale unit-price basis without an authoritative plan. Country is
correlated and recalculates tax/shipping; the current Pricing API has no country selector. Missing
shipping vector does not override scalar null, group null/empty clears explicitly and resolved
overlap refuses. Requested country/null cannot become the region's first country.

Context mutation is active-only. Already checking-out/completed checkout recovers before considering
new context or prices; snapshot prepare delegates atomic owner preparation. Hypothetical projection
and equality checks are neither live acknowledgements nor mutation authorization. A failed read
following commit remains an unknown outcome, not permission to replay or rotate mutation keys.
No schema/history rewrite, cross-owner hold, monotonic revision, durable Cart receipt/outbox or
runtime certification is supplied. See the [source handoff](../../crates/modules/rustok-commerce/docs/implementation-plan.md#atomic-cart-context-adoption-2026-10-31).

## Inventory reservation adoption into Order (2026-10-31)

The existing Inventory identity port owns bounded adoption of an existing allocation into Order
lines. Commerce supplies the same selected capability used for reserve, validates complete response
correlation, then updates its own journal/checkpoint in separate transactions. Cart-line provenance
and stable Variant/Order-line UUIDs, quantities, external identities and saved locations are retained;
no fresh Pricing/Product eligibility refresh or allocation fallback is permitted.

An unverified owner handoff is not classified as no-effect from its retry hint. Retryable failures
retain same-identity recovery; nonretryable uncertainty uses the existing lease-fenced
`reconciliation_required` journal/park/admission-event path without automatic compensation. Final
staged storefront projection keeps advice through nested recovery/journal errors without emitting
private payloads. Existing staged REST/GraphQL/native envelopes and buyer DTOs remain unchanged.
Local replay read failures are not proof of no prior owner effect; recovery preserves their bounded
classification and original typed cause before compensation lookup. Buyer errors retain existing
public codes/messages; bounded owner codes remain journal/diagnostic facts. Positive deadline
admission precedes provider polling. Full preserved metadata must fit the physical 32 KiB backend
limit before any binding write; deterministic size refusal is nonretryable, never automatic truncation.

Known previous adoption and explicit lossy-compaction provenance are preserved, not backfilled;
unknown contradictions refuse. No schema/migration/receipt/provider or reservation event publisher
is added. See the [owning source record and remaining gates](../../crates/modules/rustok-commerce/docs/implementation-plan.md#inventory-adoption-source-evidence).
This is source work, not Rust, SQL/contention, mounted/remote execution or step-8 certification.

## Security and Context Contract

Each API path must operate through a single host/runtime context:

- tenant resolution
- request-scoped locale
- auth/session handling
- request-scoped `ChannelContext`, including `resolution_source` and `resolution_trace` for channel-aware runtime diagnostics; for host resolution, `target_type`/`target_value` identify the concrete web-domain target that matched the effective host
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
