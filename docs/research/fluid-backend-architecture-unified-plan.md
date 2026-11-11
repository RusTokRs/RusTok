---
id: doc://docs/research/fluid-backend-architecture-unified-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Unified Fluid Backend Architecture (FBA) Implementation Plan for RusTok

This document is the **single current FBA implementation plan** for RusTok.
It replaces scattered planning materials. Section 2.4 defines the active embedded-first execution focus; later extraction stages are gated future work, not an instruction to create remote services now.

Related concept document: [Fluid Backend Architecture for RusTok](./fluid-backend-architecture.md).

The [Fluid Frontend Architecture (FFA)](./fluid-frontend-architecture.md) and [Fluid Backend Architecture (FBA)](./fluid-backend-architecture.md) bundle gives RusTok module portability between embedded and headless/remote profiles without rewriting core logic.


## 0) Visual Admin Context (UI parity)

Below are illustrations of two admin runtime variants that must preserve the same
business semantics and navigation contract under FFA+FBA.

### Leptos variant (SSR-first)

![Leptos Admin Dashboard](https://github.com/user-attachments/assets/leptos-admin-dashboard)

*Description:* dark SSR-first admin dashboard with blocks `Total users / Content nodes / Orders / Revenue snapshot`,
`Recent activity` block and `Enabled modules` widget. This is the reference for the Leptos-host profile
(`apps/admin`) and module-owned UI route contract.

### Next variant (headless/runtime parity)

![Next Admin Dashboard](https://github.com/user-attachments/assets/next-admin-dashboard)

*Description:* Next-host variant with the same semantic set: metric dashboard, activity,
operator actions and modular navigation. This is the reference for `apps/next-admin`, which must
maintain parity with the Leptos variant in data, roles and scenarios.

> Note: visual style may differ, but the FFA+FBA contract requires stability
> of domain scenarios, access rights, route/query semantics and backend orchestration behavior.

---

## 1) Purpose and Boundaries

## 1.1 Purpose

Move individual module boundaries into a remote execution profile (e.g., gRPC/async worker) **without rewriting domain/application logic**.

## 1.2 FBA Architectural Invariant

All stages preserve:

- `module identity` (slug/ownership/responsibility area);
- `service contract` (commands, queries, events);
- `domain rules` and policy semantics.

For an explicitly supported profile, topology and adapters change while owner semantics
remain stable. The profile must name its operations and outgoing dependencies. Remote
capability coverage does not imply complete module extraction, equivalent transactions,
autonomous storage or current UI/API parity. The
[canonical FBA contract](../backend/module-backend-architecture.md#fba-contracts)
defines these distinctions. Each native module keeps one owner identity, with
published contracts in its nested `ports/` package and implementation in the root package.

## 1.3 What Is Prohibited

- "Each crate = a microservice".
- Duplicating business logic across transport handlers.
- Direct access to foreign tables after port formalization.
- Premature migration to service-owned DB before ports/events/observability are mature.

---

## 2) Stage Structure and Execution Gates

1. **Stage A — Module Audit and Readiness**
2. **Stage B — Basic FBA Contracts (Before Transport)**
3. **Stage C — Event Discipline and Contract Testing**
4. **Stage D — Pilot 1 (async/read-boundary)**
5. **Stage E — Pilot 2 (Inventory Reservation)**
6. **Stage F — Pilot 3 (Payment/Fulfillment/Product read/Pricing)**
7. **Stage G — Selective Storage Decomposition and Write Extraction**

The labels preserve the planning sequence, but named Inventory/Product examples do not
override the active embedded-first focus or schedule remote deployment. For any selected
pilot, complete its contract/event/consistency prerequisites before transport and storage
cutover. The Media/Search proposal has its own candidate scope and acceptance gates.

## 2.1 Current FBA Tracks and Unified Template

Current statuses come from owner-local implementation plans and are projected into the
[central readiness board](../modules/registry.md#ffafba-readiness-board-module-owned-ui).
This plan does not maintain a third independent status table. Each track uses neutral
ports, provider/consumer metadata, typed errors, source guards and explicitly scoped
execution/failure evidence.

Track entrypoints include:

- Product catalog reads: `ProductCatalogReadPort`,
  `crates/modules/rustok-product/contracts/product-fba-registry.json` and the
  [Product plan](../../crates/modules/rustok-product/docs/implementation-plan.md).
- Ecommerce orchestration and owner ports: the [central registry](../modules/registry.md)
  links Commerce, Pricing, Inventory, Customer, Cart, Order, Payment, Fulfillment and Tax.
- `blog` consumes `CommentsThreadPort` through
  `crates/modules/rustok-blog/contracts/blog-fba-registry.json`.
- `ai-product` uses `ai-product-fba-registry.json` and
  `ai-product-runtime-fallback-smoke.json` under its owner `contracts/` directory.
- `ai-media` consumes `MediaAssetReadPort` with `ai-media-fba-registry.json` and
  `ai-media-runtime-fallback-smoke.json` under its owner `contracts/` directory.
- `ai-alloy` records `alloy_script_execution_policy` in `ai-alloy-policy-registry.json`.
- Page Builder/Pages, Media/SEO/Profiles, Search and foundation capabilities retain their
  own registries and plans; their status and verification scope must be read there.

Evidence whose filename contains `runtime-smoke` may be a source/metadata check only.
Read its execution status and commands before claiming a real owner invocation.

Uniformity rules:

1. **FBA remains the rollout name, not a mandatory type prefix.** Code-facing contracts use neutral names (`PortContext`, `PortError`, `*Port`, `provider`, `consumer`).
2. **Status source is local `docs/implementation-plan.md`, the central board is synced in the same change.** Do not leave `not_started` if there is active FBA provider/consumer evidence.
3. **Machine-readable metadata is mandatory for provider/consumer tracks.** For `page_builder -> pages` the source is `page-builder-fba-registry.json`; subsequent tracks must reuse the same format or explicitly extend it in this plan, not create a parallel format.
4. **Neutral port primitives apply only to new/updated ports.** Existing FBA slices are not mechanically rewritten without feature work; upon the next change they are brought to the same `context/error/idempotency/deadline` requirements.
5. **Promotion to `boundary_ready` or `transport_verified` requires evidence.** Metadata or FFA split alone does not count as remote/runtime verification.

## 2.2 Structural Standard for Module Migration

### Proposed whole-module extraction pilots: media and search

The Media/Search proposal targets whole-module deployments rather than a separate
microservice per port. It remains Proposed; existing bounded adapters are current
implementation and do not establish that the proposed isolated services exist. `media` is the storage/read pilot and
`search` is the query/connector pilot. The modular monolith remains the default.
Media loopback transport conformance is accepted; an actual process split still
requires isolated-runtime evidence.

`rustok-search` owns the `SearchEngine` connector abstraction. PostgreSQL,
Meilisearch, Typesense, and Algolia connectors are internal adapters selected
by the search service; storefront/admin consumers call only
`SearchQueryPort` and `SearchSuggestionPort`.

The former Index v1 document-ingestion/read-model boundary was removed
completely. The replacement `rustok-index` is a generic cross-module relational
Index Engine whose query/rebuild contracts remain `in_progress`; Search integration
and any remote ingestion split are deferred until the replacement contracts have
compiled and live replay, lag, rebuild, and recovery evidence.

The extraction order is:

1. contract and error-matrix hardening;
2. generic gRPC loopback conformance for the existing ports;
3. isolated `media` process/database/storage proof;
4. isolated `search` query process with internal connector selection;
5. optional index-ingestion split after replay, duplicate, lag, rebuild, and
   recovery evidence;
6. performance and operational comparison against the in-process baseline.

The proposed decision and required acceptance evidence are recorded in
`DECISIONS/2026-07-16-media-search-extraction-boundaries.md`. No other module
is promoted to a remote deployment class by this plan.

### Extraction execution status

Product has a standalone host for catalog projections and SKU identity. Commands stay
embedded and additional read capabilities remain open. Media has metadata/control and
public-image descriptor adapters plus a narrow Profiles consumer selector, but no
standalone `rustok-media-service` package or whole-module runtime selector. The
[owner plans](../modules/registry.md) record actual coverage and open evidence.
The July 16 Media/Search ADR remains a proposal, not an accepted completion record.

The deployment default remains the modular monolith. Media now has its bounded
metadata/control gRPC adapter and compiled loopback conformance. This does not
authorize a production process/database split without the remaining operational
evidence.

Resume only after the platform baseline has explicit evidence for the affected
scope: reproducible targeted compilation/tests, a resolved `Cargo.lock`
baseline, owner API contracts with typed error behavior, and no known
cross-module database/service access on the paths being changed.

When the freeze is lifted, implement and verify the following order:

1. Re-audit `rustok-fba` and `rustok-runtime`; add a shared transport or
   conformance primitive only after two real module consumers demonstrate the
   same need.
2. Complete Media owner write/control coverage for upload, delete,
   translations, and reconciliation. Keep large binary data on Media-owned streaming
   REST or a presigned upload flow; do not put blobs in generic gRPC DTOs.
3. Implement the internal Search connector writer and owner ingestion/control
   contract for schema synchronization, document upsert/delete, rebuild, and
   health. Connectors remain private to `rustok-search`.
4. Remove Search query-time SQL access to the removed Index v1 projection
   tables. Populate the needed category and facet fields in Search-owned
   projections during ingestion. Do not depend on removed Index v1 ports;
   reconnect only through replacement Index Engine contracts after the first
   vertical slice.
5. Media loopback gRPC conformance is complete. Add isolated database/storage
   evidence for Media, then add the isolated Search query-service profile.
6. Promote no readiness status without compiled and live evidence for tenant,
   authorization, retry/restart, health/metrics, degraded behavior, and
   rollback/recovery.

Yes, there is a single standard. For each new FBA increment the same
artifact structure is mandatory; absence of any item below is considered a gap and does not allow
status above `in_progress`:

1. **Local source of truth:** `crates/modules/<module>/docs/implementation-plan.md` contains
   `## FFA/FBA status`, current role (`provider`, `consumer`, `orchestrator`, `support`)
   and evidence on boundary/metadata/verification.
2. **Central status:** `docs/modules/registry.md` contains a synchronized readiness board row
   with the same FBA status and a link to the local plan.
3. **Runtime metadata:** `rustok-module.toml` or module-owned machine-readable registry
   captures the provider/consumer dependency profile, contract versions, degraded modes and
   toggle/fallback profiles if the module participates in a provider/consumer track.
4. **Contract location:** published DTO, port and error contracts live in the owner's
   nested `<owner-package>-ports` package. Shared platform primitives belong in foundation
   crates. Host composition uses published contracts without taking domain ownership.
5. **Verification location:** alongside machine-readable metadata there is an anti-drift/fallback gate
   (`scripts/verify/*` or module-local verifier), and the local plan lists command/evidence.
6. **Evidence packet:** for Wave/pilot rollout there are actual or explicitly marked
   synthetic before/after snapshots, smoke outcomes, metrics/traces and keep/rollback decision.
7. **Docs sync:** if FBA status, provider/consumer metadata, ports/events, routing,
   tenancy, UI contract or observability changes, local docs, central board
   and this unified plan are updated simultaneously if the standard itself changes.


### Historical increments

The dated entries below retain historical scope and reported results. They do not
supersede current owner plans or certify the current revision after later contract changes.

As of 2026-06-16 the ecommerce provider track additionally received a unified static evidence layer for future contract tests: `pricing`, `inventory`, `order`, `payment`, `fulfillment`, `customer` and `cart` have `contracts/evidence/*-contract-test-static-matrix.json`, the command `npm run verify:ecommerce:fba` runs registry + evidence gates, including checking evidence packet compliance with provider registry cases/fallback profiles via `npm run verify:ecommerce:fba-contract-evidence`. This still does not promote status to `boundary_ready`: runtime execution and fallback smoke remain separate gates.

As of 2026-06-29 `payment` and `fulfillment` have been promoted to `boundary_ready` based on provider SPI live-adapter executed evidence: `crates/modules/rustok-payment/contracts/evidence/payment-provider-spi-live-adapter-evidence.json` and `crates/modules/rustok-fulfillment/contracts/evidence/fulfillment-provider-spi-live-adapter-evidence.json` capture concrete external adapter contract execution for guarded invocation, typed provider-error mapping, degraded fallback, unavailable-mode blocking and webhook/tracking replay delegation. Base port contract/fallback smoke for `PaymentCollectionPort` and `ShippingSelectionPort` remains follow-up before `transport_verified`.

As of 2026-06-18 the fast gate `npm run verify:ecommerce:fba-registries` additionally checks in-process provider implementations at each operation level, if the registry declares `in_process_provider_impl`: read operations must call `require_deadline_semantics()?`, write operations with `idempotency_required = true` must call `require_write_semantics()?`, and read operations must not accidentally require write-idempotency. This closes anti-drift for typed context/deadline/idempotency semantics without running expensive compilation, but also does not promote status without runtime contract execution. For `order.checkout_completion.v1` the same fast gate now additionally blocks premature `OrderService` in-process implementation (both registry metadata and source-level impl) until the registry contains `runtime_evidence.checkout_completion_owner_path.status = "runtime_verified"`; this protects the checkout completion boundary from a fake embedded provider without cart/result projection evidence.

As of 2026-06-29 `order` is promoted to `boundary_ready`: `crates/modules/rustok-order/contracts/order-fba-registry.json` contains `runtime_evidence.checkout_completion_owner_path.status = "runtime_verified"`, and `npm run verify:ecommerce:fba` checks the owner `OrderService` implementation for write/read policy, lifecycle delegation, locale-aware snapshot reload and typed unavailable result projection. Remote/base fallback smoke remains a condition before `transport_verified`.

As of 2026-06-18 `product` added to the ecommerce provider track as a catalog read provider: `ProductCatalogReadPort`/`product.catalog_read.v1`, registry `crates/modules/rustok-product/contracts/product-fba-registry.json` and static evidence `crates/modules/rustok-product/contracts/evidence/product-contract-test-static-matrix.json` are included in `npm run verify:ecommerce:fba` together with product dependency in the commerce consumer registry.


As of 2026-06-18 `customer` added to the ecommerce provider track as a customer read-projection provider: `CustomerReadPort`/`customer.read_projection.v1`, registry `crates/modules/rustok-customer/contracts/customer-fba-registry.json` and static evidence `crates/modules/rustok-customer/contracts/evidence/customer-contract-test-static-matrix.json` are included in `npm run verify:ecommerce:fba` together with customer dependency in the commerce consumer registry.


`cart` is an ecommerce provider through `CartCheckoutPort`/`cart.checkout.v2`, covering checkout snapshots and lifecycle writes. Its registry and static evidence are included in `npm run verify:ecommerce:fba` together with the cart dependency in the commerce consumer registry.

As of 2026-06-18 `tax` added as a support provider track for cart tax calculation: `TaxCalculationPort`/`tax.calculation.v1`, registry `crates/modules/rustok-tax/contracts/tax-fba-registry.json` and static evidence `crates/modules/rustok-tax/contracts/evidence/tax-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:tax:fba` without promotion to `boundary_ready` until runtime execution/fallback smoke.

As of 2026-06-30 the batch commerce-domain wave added no-compile runtime-order evidence for `product`, `pricing`, `inventory`, `customer`, `cart` and `tax`: module-owned `contracts/evidence/*-runtime-contract-smoke.json` are checked by the shared `scripts/verify/verify-commerce-domain-fba-runtime-smoke.mjs` with fixture regressions. The gate fixes the order of shared policy/idempotency -> owner provider -> typed error mapping and parity fallback/degraded metadata, but intentionally does not promote modules above `in_progress` until live provider execution.

As of 2026-06-30 the owner-provider wave added a unified no-compile runtime-order gate for `comments`, `rbac`, `workflow`, `region`, `media` and `outbox`: `contracts/evidence/*-provider-runtime-order-smoke.json` and `scripts/verify/verify-owner-fba-runtime-order.mjs` capture shared policy, idempotency, tenant/request validation, owner invocation/evaluation and typed error mapping. `comments` uses canonical `PortCallPolicy::write()`, which already applies write semantics inside the shared primitive. As of 2026-07-01 `outbox` registry/manifest migrated to the single `rustok_api::ports` contract, and the outbox-specific adapter moved to the owner crate, eliminating the reverse runtime dependency from `rustok-api`. Statuses are not promoted without live execution.

As of 2026-06-30 the orchestrator/capability wave added a unified no-compile runtime-order gate for `ai` and `page_builder`: `contracts/evidence/*-orchestrator-runtime-order-smoke.json` and `scripts/verify/verify-orchestrator-fba-runtime-order.mjs` capture AI support-adapter registry parity, direct runtime registration APIs, router fallback diagnostics markers, native/GraphQL admin transport boundary, and page-builder capability flag -> `PortCallPolicy` -> owner service call and authorization -> service call order. Statuses remain `in_progress` until live runtime/tenant evidence.

The consumer wave uses a unified no-compile runtime-order gate for `blog` and `seo`. SEO composes `MediaAssetReadPort` through `SeoMediaAssetReadProvider`; Product supplies its media UUID, while URL-only target records keep their owner descriptor. The static gate does not substitute for tenant/degraded consumer runtime execution, so statuses remain `in_progress`.

As of 2026-06-18 `comments` added as a provider track for generic comment threads: `CommentsThreadPort`/`comments.thread.v1`, registry `crates/modules/rustok-comments/contracts/comments-fba-registry.json` and static evidence `crates/modules/rustok-comments/contracts/evidence/comments-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:comments:fba` without promotion to `boundary_ready` until runtime execution/fallback smoke.

As of 2026-06-19 `search` added as a provider track for search query/suggestions boundary: `SearchQueryPort`/`SearchSuggestionPort`/`search.query.v1`, registry `crates/modules/rustok-search/contracts/search-fba-registry.json` and static evidence `crates/modules/rustok-search/contracts/evidence/search-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:search:fba`; as of 2026-06-29 executable no-compile runtime fallback/contract/invocation evidence promoted FBA status to `boundary_ready`, and live runtime contract execution remains a condition for `transport_verified`.

As of 2026-06-19 `blog` added as a consumer track for `comments.thread.v1`: registry `crates/modules/rustok-blog/contracts/blog-fba-registry.json`, static evidence `crates/modules/rustok-blog/contracts/evidence/blog-comments-consumer-static-matrix.json`, no-compile source-smoke `crates/modules/rustok-blog/contracts/evidence/blog-comments-runtime-fallback-smoke.json`, manifest `[fba.consumer]` and fast gate `npm run verify:blog:fba` capture `CommentsThreadPort` dependency, fallback profiles and degraded modes without promotion to `boundary_ready` until real runtime execution.

As of 2026-07-22 `media` has compiled embedded/loopback runtime conformance for every `MediaAssetReadPort` and `MediaAssetWritePort` operation in `crates/modules/rustok-media-transport/tests/port_conformance.rs`. The fast `npm run verify:media:fba` gate locks this runtime evidence to the provider registry and static matrix; FBA remains `boundary_ready` until isolated deployment evidence supports `transport_verified`.

The `seo` consumer track for `media.asset_read.v1` is recorded in `crates/modules/rustok-seo/contracts/seo-fba-registry.json`. Product forwards its media asset UUIDs, which SEO resolves through the composed `MediaAssetReadPort`. Other target providers still return URL-only descriptors. The static matrix and `npm run verify:seo:fba` check this wiring and the declared fallback profiles. Promotion to `boundary_ready` requires live provider and degraded consumer execution.

As of 2026-06-20 `ai-media` has a support-consumer track for `media.asset_read.v1`: registry `crates/modules/rustok-ai-media/contracts/ai-media-fba-registry.json`, static evidence `crates/modules/rustok-ai-media/contracts/evidence/ai-media-consumer-static-matrix.json`, runtime fallback smoke `crates/modules/rustok-ai-media/contracts/evidence/ai-media-runtime-fallback-smoke.json` and gate `npm run verify:ai-media:fba` capture `MediaAssetReadPort` dependency, adapter source markers, fallback profile and degraded mode; as of 2026-06-29 support-adapter smoke confirmed by `cargo test -p rustok-ai-media --lib` and status promoted to `boundary_ready`. `ai-alloy` added as a source-level policy track: `crates/modules/rustok-ai-alloy/contracts/ai-alloy-policy-registry.json` and `alloy_script_execution_policy` capture ownership of script runtime payload policy, allowed operations and descriptor runtime operation while runtime composition stays in `rustok-ai`.

As of 2026-06-19 `workflow` added as a provider track for admin read-projection boundary: `WorkflowReadPort`/`workflow.read_projection.v1`, registry `crates/modules/rustok-workflow/contracts/workflow-fba-registry.json` and static evidence `crates/modules/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:workflow:fba` without promotion to `boundary_ready` until runtime execution/fallback smoke.

As of 2026-06-19 `rbac` added as a provider track for admin permission-decision boundary: `RbacPermissionDecisionPort`/`rbac.permission_decision.v1`, registry `crates/modules/rustok-rbac/contracts/rbac-fba-registry.json` and static evidence `crates/modules/rustok-rbac/contracts/evidence/rbac-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:rbac:fba` without promotion to `boundary_ready` until runtime fallback smoke.

As of 2026-06-20 `tenant` added as a provider track for tenant read-projection boundary: `TenantReadPort`/`tenant.read_projection.v1`, registry `crates/modules/rustok-tenant/contracts/tenant-fba-registry.json` and evidence `crates/modules/rustok-tenant/contracts/evidence/tenant-contract-test-static-matrix.json` are checked by gate `npm run verify:tenant:fba`; as of 2026-06-29 runtime contract/fallback smoke executed via `cargo test -p rustok-tenant --lib --tests`, evidence marked `runtime_verified`, and status promoted to `boundary_ready`; the current no-compile package `scripts/verify/verify-foundation-fba-runtime-smoke.mjs` adds source-locked fallback smoke `crates/modules/rustok-tenant/contracts/evidence/tenant-runtime-fallback-smoke.json` without running compilation.

As of 2026-06-20 `region` added as a provider track for region/country read-projection boundary: `RegionReadPort`/`region.read_projection.v1`, registry `crates/modules/rustok-region/contracts/region-fba-registry.json` and static evidence `crates/modules/rustok-region/contracts/evidence/region-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:region:fba` without promotion to `boundary_ready` until runtime contract/fallback smoke.

As of 2026-06-20 `channel` added as a provider track for channel/default/host-target read-projection boundary: `ChannelReadPort`/`channel.read_projection.v1`, registry `crates/modules/rustok-channel/contracts/channel-fba-registry.json` and static evidence `crates/modules/rustok-channel/contracts/evidence/channel-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:channel:fba`; as of 2026-06-29 no-compile executable fallback smoke promoted FBA status to `boundary_ready`, and full Rust runtime contract/fallback evidence remains a condition for `transport_verified`.

On 2026-06-20 Index v1 introduced a read-model/rebuild provider track. The destructive Index Engine rewrite removed that track completely: ports, contract identifiers, registry, evidence, verifier integration, projections, and runtime wiring no longer exist. Replacement Index FBA readiness is `in_progress` until new query/rebuild contracts and live provider-consumer evidence are published.

As of 2026-06-20 `outbox` added as a provider track for relay worker control boundary: `OutboxRelayPort`/`outbox.relay_control.v1`, registry `crates/modules/rustok-outbox/contracts/outbox-fba-registry.json` and static evidence `crates/modules/rustok-outbox/contracts/evidence/outbox-contract-test-static-matrix.json` are checked by the fast gate `npm run verify:outbox:fba` without promotion to `boundary_ready` until runtime contract/fallback smoke.

As of 2026-06-20 `email` added as a provider track for transactional delivery boundary: `EmailDeliveryPort`/`email.delivery.v1`, registry `crates/modules/rustok-email/contracts/email-fba-registry.json` and evidence `crates/modules/rustok-email/contracts/evidence/email-contract-test-static-matrix.json` are checked by gate `npm run verify:email:fba`; as of 2026-06-29 targeted delivery-port tests executed via `cargo test -p rustok-email --lib`, disabled-provider noop fallback marked `runtime_verified`, and status promoted to `boundary_ready`; the current no-compile package `scripts/verify/verify-foundation-fba-runtime-smoke.mjs` adds source-locked fallback smoke `crates/modules/rustok-email/contracts/evidence/email-runtime-fallback-smoke.json` for disabled noop/SMTP delivery profiles without running compilation.

The structure check on the current state revealed one fixed gap: `page_builder` already
had FBA provider metadata and registry but was absent from the readiness board and had no local
FFA/FBA status block. Now `page_builder` and `pages` are reflected uniformly: local plan +
central board + machine-readable registry/evidence. Remaining gaps are not a violation of the
standard because they are explicitly recorded as `not_started`/`deferred` or as compile/runtime
blockers in verification output.

## 2.3 Alignment with Target Crate Structure

The [canonical source layout](../backend/module-backend-implementation.md#remote-fba-module-layout)
applies to both new and converted native owners. This view shows where each host
fits and how existing deployment packages converge on the target:

```text
crates/modules/rustok-<module>/
  src/dto|domain|error      # private owner types; published DTOs and errors live in ports/
  src/services|ports        # owner services and implementation-local requirements
  src/entities|migrations   # owner storage and lifecycle
  src/graphql|controllers   # thin owner HTTP and GraphQL adapters
  src/bin/service.rs        # standalone composition, behind the service feature
  ports/                    # <owner-package>-ports: published ports, DTOs, errors and events
    src/grpc/               # optional gRPC client/server adapters for implemented RPCs
    proto/                  # published wire schema for those operations
  admin|storefront          # optional module-owned UI packages with core/transport/ui split
  rustok-module.toml        # runtime metadata, dependencies, provider/consumer FBA sections
  contracts/                # optional machine-readable registry/evidence for provider/consumer tracks

apps/server/
  composition/root wiring   # module registry, GraphQL/REST/controllers, health/metrics
  transport selection       # explicit supported profile, e.g. bounded Product catalog reads
  public API                # host API does not own domain rules
```

Key differences from the `service trait + in-process impl + repository interfaces` schema as a
mandatory template:

1. **Trait-port is not introduced mechanically.** If a module is not yet ready for remote/profile split,
   the service struct remains a valid owner service layer; trait/adapter is extracted on the first
   real boundary or contract-test increment.
2. **Repository interfaces are not mandatory from the first PR.** Currently many modules own SeaORM
   entities/migrations directly; the abstraction seam is added when a remote adapter,
   test double or foreign-table access prohibition is needed.
3. **gRPC is optional within `ports/`.** Publish the owner contract for a real cross-owner
   boundary. Add gRPC for implemented request/response profiles, with stable types, context,
   authorization and conformance evidence. Existing Product and Media `*-transport` packages
   are migration states; preserve their wire contracts while moving the adapters.
4. **Transport adapters already exist, but not all are remote.** GraphQL/REST/`#[server]` live as
   thin adapters on top of owner service/port; gRPC will be another adapter profile, not a new
   business logic implementation.
5. **Machine-readable provider/consumer metadata is already part of the structure.** For
   `page_builder -> pages` this is `page-builder-fba-registry.json` + `rustok-module.toml`; new
   provider/consumer tracks must repeat this pattern or extend it within the unified plan.

The nested ports package belongs to the same module identity. Runtime readiness
requires independent compile and operation evidence. Add source for implemented responsibilities.

### Converge existing owners on published ports

1. Run `python scripts/architecture_dependency_guard.py --fba-inventory` to locate
   direct runtime-package edges. Inventory real consumers, foreign SQL and native
   shared transactions in each owner plan. A dependency edge is a review candidate.
   Separate neutral shared facilities, such as the outbox producer contract, from
   independently deployed owner runtimes.
2. Publish narrow owner ports and move consumers to explicit host bindings. Preserve
   deployed wire identities and eliminate duplicate authoritative data. Use the same
   owner port and workflow in the monolith and across process boundaries.
3. Establish the data, migration and transaction boundary, including attached
   values owned by donors and schemas/policies owned by their defining capability.
   Replace cross-owner SQL transactions only with verified equivalent operation
   guarantees or an explicit business consistency decision.
4. Verify the lightweight contract/client build, embedded execution and standalone
   profile for each migrated scope. Update its owner plan and the existing readiness
   board together. Deploy independently according to product and operational needs.

Product, Flex, Inventory and Taxonomy have existing shared-storage and host binding
work to reconcile. Media's bounded read profile offers a smaller packaging slice.
Complete provider contracts before converting their multi-owner orchestrators.

## 2.4 Active Embedded-First Foundation Plan (2026-H2)

### Current Execution Focus

The active platform priority is to make owner modules production-ready in the
modular monolith. FBA is applied now as boundary discipline, not as a mandate
to deploy modules remotely. Embedded execution remains the default while each
owner works toward verified embedded and independent execution.

This focus does not change the later stages in this document or authorize a
remote deployment. It makes their prerequisites real while improving the
current product: module ownership, tenant safety, operator behaviour, UI
parity, observable failure handling, and recovery all have value before any
process boundary exists.

### Scope

For each production-priority module, complete the following work in its own
implementation plan before treating a new technical extraction as a priority:

1. **Finish the owner path.** The module owns its domain rules, persistence,
   migrations, events, authorization decisions, and operational recovery. No
   host controller or neighbouring module becomes the owner of those rules.
2. **Ship its required product surfaces.** Keep the applicable native
   `#[server]`, GraphQL, REST, admin, storefront, and operator contracts in
   their documented parity state. A transport remains a thin adapter over the
   owner service or an existing real port.
3. **Harden real boundaries.** When a module has a genuine cross-module,
   host, CLI, or headless consumer, use the canonical `PortContext`,
   `PortCallPolicy`, and `PortError`; validate deadline and idempotency at the
   owner boundary; and prohibit direct foreign-table access on that path.
   Do not introduce a port merely to satisfy a template.
4. **Record executable evidence.** Verify the embedded owner path with
   targeted tests or a real smoke scenario. Source markers and static metadata
   can prevent drift, but they do not establish production readiness on their
   own.
5. **Keep the FBA record honest.** For an existing provider/consumer track,
   synchronize the module implementation plan, `rustok-module.toml` or
   module registry, evidence packet, and the central readiness board in the
   same change. Do not promote a status for work that has not executed.

### Foundation Improvements Allowed During This Focus

Foundation work is allowed only when it removes demonstrated repetition or
protects a current embedded production path. It must remain transport-neutral
and cannot introduce a second implementation of a domain workflow.

| Improvement | Current value | Entry condition | Completion evidence |
|---|---|---|---|
| Canonical FBA registry validation | Prevent malformed provider/consumer metadata and stale evidence links. | At least two registries require the same structural rule that is currently duplicated in verifier scripts. | One shared schema/validator plus module-specific semantic checks; existing module gates continue to pass. |
| Embedded port conformance harness | Run the same observable owner-port cases consistently across modules. | A second owner module needs the same real conformance cases as the first. | Targeted tests exercise the real embedded provider, tenant context, deadline, idempotency, and typed errors. |
| In-process tracing propagation | Keep correlation, causation, and trace context intact across module calls. | A current cross-module path lacks trace continuity or cannot be diagnosed from existing spans. | A smoke scenario demonstrates one trace across the caller and owner service without exposing sensitive data. |
| Shared policy helper | Remove repeated, identical enforcement of an already-canonical port policy. | The exact helper exists in two owners and cannot remain on `PortContext` itself. | Both owners use one helper and preserve the same rejection and error semantics in tests. |
| Error-mapping test utilities | Keep REST, GraphQL, native, and owner-port errors consistently classified. | Two adapters duplicate the same `PortError` assertion or mapping logic. | Real adapter tests prove the same public error contract; domain error ownership remains unchanged. |

The shared location follows ownership: stable transport-neutral contracts in
`crates/libs/rustok-api`, executable runtime helpers in
`crates/libs/rustok-runtime`, Axum boundary helpers in
`crates/libs/rustok-web`, and FBA metadata in `crates/libs/rustok-fba`.
Do not create a new foundation crate unless the responsibility cannot belong
to one of those existing owners.

### Explicitly Deferred

The following are not active work under the embedded-first focus:

- new `*-grpc` adapter crates, remote profile switching, or a general remote
  invocation pipeline;
- process, database, or storage separation for an owner module;
- shared remote retry, circuit-breaker, or load-shedding infrastructure;
- a dynamic provider-discovery mechanism that bypasses the selected
  distribution registry;
- introducing WebAssembly/WIT solely to make an embedded module appear more
  portable.

Existing transport and worker boundaries remain supported only within their
current owner scope. They are not a precedent to create a remote adapter for a
new module.

### Completion and Re-entry Gate

This execution focus remains active until the production-priority modules and
their documented embedded surfaces have current owner-path evidence. It is not
ended by the presence of ports, registries, or static verifier output alone.

Remote-profile work may resume only for a named module when all of the
following are true:

1. the module satisfies the Definition of Ready in section 10;
2. there is a production reason that cannot be met by the embedded profile;
3. an ADR names the owner, operational cost, failure model, rollout,
   rollback, and storage impact;
4. the module has a real embedded conformance baseline against which the new
   profile can be compared; and
5. the work is explicitly scheduled as a platform priority.

Until then, `boundary_ready` means a well-defined and evidenced embedded
boundary. It does not mean that the module should be deployed as a separate
service.

---

## 3) Stage A — Audit and Readiness Matrix

## 3.1 Mandatory Artifacts

- `Module Inventory Table` (for each target module):
  - slug, owner, owned storage, public use-cases;
  - incoming/outgoing events;
  - dependencies (Cargo + modules graph);
  - role: orchestrator/facade, write-model owner, read-model provider, support service.
- `Coupling Debt Register`:
  - direct calls to neighboring domains;
  - direct SQL to foreign tables;
  - missing idempotency/deadline;
  - event gaps (no outbox/versioning/replay policy).
- `Readiness Matrix`: High / Medium / Low.

## 3.2 Stage A Exit Criteria

- All modules in the target scope have a filled inventory row.
- For each Medium/Low module a remediation backlog is recorded.
- For each remote candidate there is an ADR draft with risks and rollback approach.

---

## 4) Stage B — Basic FBA Contracts (Ports before transports)

## 4.1 Unified `PortContext`

The initial shared implementation lives in `rustok-api::ports` and intentionally remains transport-agnostic: it is a contract primitive for ports/adapters, not a domain service.

Mandatory fields:

- tenant;
- actor/service identity;
- claims/role;
- channel + locale;
- correlation/causation + trace context;
- idempotency key (write);
- deadline/timeout/cancellation.

Rule: passed as an explicit parameter of each port.

## 4.2 Unified Error Model

A single set of domain errors (validation/not_found/conflict/forbidden/unavailable/timeout/invariant violation) + predictable mapping to REST/GraphQL/gRPC.

## 4.3 Port Layer

Minimum target set of ports:

- `ProductPort`, `PricingPort`, `InventoryPort`, `CartPort`,
- `OrderPort`, `PaymentPort`, `FulfillmentPort`, `TaxPort`.

Requirement: in-process impl first, then remote adapters.

## 4.4 Data Ownership Policy

- A module reads/writes only its own storage.
- Cross-module data access — only via port/snapshot DTO/read model.

## 4.5 Stage B Exit Criteria

- All target ports defined in transport-agnostic form.
- `PortContext` and error model used in all new/updated port calls.
- New direct foreign-table accesses are not allowed.

---

## 5) Stage C — Events, Outbox and Contract Testing

## 5.1 Event Vocabulary

For critical domains define a versioned vocabulary (e.g.: `ProductPublished`, `PriceChanged`, `InventoryReserved`, `OrderPlaced`, `PaymentAuthorized`).

Each event must have: tenant, aggregate id, schema version, correlation/causation, idempotency semantics.

## 5.2 Outbox Discipline

- Write domain state + outbox in one transaction.
- Publish via worker/dispatcher.
- Consumers are idempotent + replay-safe + tolerant to out-of-order.

## 5.3 Contract Tests

For each declared remote port/profile, the same observable operation cases run:

- against in-process impl;
- against remote adapter.

Business results and owner policy must agree for supported operations. Add explicit network
failure, timeout/uncertain-write, cancellation and recovery cases. An optional method with
no implementation is an unsupported capability, not conformance coverage. Historical
attestation and source-only checks do not certify the current revision.

## 5.4 Stage C Exit Criteria

- Outbox is enabled for all write owners in the pilot scope.
- Contract tests exist for all ports in the pilot scope.
- There are replay/idempotency/out-of-order scenarios in tests.

---

## 6) Stage D — Pilot 1 (async/read-boundary)

## 6.1 Candidates

- search/indexing;
- AI enrichment/recommendations.

## 6.2 Steps

1. Extract boundary into port and adapter (gRPC or async worker — depending on use-case nature).
2. Wire up embedded/remote switching via runtime config.
3. Move host/facade calls to the port.
4. Verify SLO: latency, error rate, throughput, retry behavior.

## 6.3 Exit Criteria

- Functional parity with embedded profile confirmed.
- Metrics and tracing stable for at least the agreed observation window.

---

## 7) Stage E — Pilot 2 (Inventory Reservation)

## 7.1 Steps

1. Introduce `reservation` model: idempotency key, TTL/expiration, status lifecycle.
2. Establish events: `InventoryReserved`, `InventoryReservationReleased`, `InventoryAdjusted`.
3. Implement `InventoryPort` remote server/client.
4. Embed compensations in the checkout saga (`release_reservation`).
5. Run load tests on peak checkout scenarios.

## 7.2 Exit Criteria

- Reservation commands are retry-safe.
- Compensations correctly handle controlled failures.
- Load profile does not degrade below agreed thresholds.

---

## 8) Stage F — Pilot 3 (Payment/Fulfillment/Product read/Pricing)

The historical sequence below describes candidate dependencies after a named remote
workstream is admitted. It does not schedule these owners ahead of section 2.4 or the
Media/Search proposal. External payment-provider APIs are also distinct from extracting
the RusTok Payment module itself:

1. `PaymentPort` and `FulfillmentPort` as remote adapters (external providers).
2. `ProductPort` read-side snapshots (`get_product_snapshot`, `list_publishable_catalog_page`).
3. `PricingPort` after product read contracts are stabilized.
4. `TaxPort` as an explicit support boundary (embedded/stateless remote/provider adapter — decided by ADR).

## 8.1 Exit Criteria

- No direct reading of product internals from pricing.
- Checkout orchestration works through ports with the same business results.
- Synchronous path and async post-processing are separated architecturally.

---

## 9) Stage G — Late Stages (Storage and Write Extraction)

Allowed storage modes:

1. shared DB + in-process;
2. controlled shared DB + remote process, with explicit owner access and schema coordination;
3. service-owned DB;
4. read-model replica/projection.

Rule: transition to service-owned storage only under an explicit approved ownership,
consistency, migration and recovery decision. The former shared SQL transaction cannot
cross RPC. Co-deploying atomic participants or adopting a new workflow requires a
documented decision; adding ports alone does not change atomicity. Existing shared-storage
read profiles must disclose that they are process extraction, not autonomous storage.

---

## 10) Unified Definition of Ready for Module Remote Migration

A named remote capability/profile can be admitted only when all applicable conditions
below are met. Whole-module deployment additionally requires a complete operation,
event/job, lifecycle/migration and public-surface inventory with resolved outgoing
dependencies and transaction boundaries:

1. Stable transport-agnostic port + contract tests (in-process/remote).
2. Full `PortContext` on all commands/queries.
3. Outbox + versioned events + replay/idempotency policy.
4. No foreign-table access outside owner boundary.
5. Write methods have idempotency key and deadline semantics.
6. Health/readiness/metrics/tracing parity between profiles.
7. Separate ADR with reasons, risks, rollback and ownership impact.

---

## 11) Illustrative Rollout Template

These are relative planning periods, not calendar commitments or authorization to end
the embedded-first focus. A named owner plan and ADR must schedule each remote increment.

- **Period 1:** Stages A+B.
- **Period 2:** Stage C + Pilot 1.
- **Period 3:** Pilot 2.
- **Period 4:** Pilot 3 + decisions on selective storage evolution.

If the selected stage Exit Criteria are not met, its dependent step does not start.

---

## 12) Document Change Management

- This document is the canonical FBA implementation plan.
- Changes to sequence/criteria are made only together with updating related ADRs.
- New "parallel FBA plans" are not created; extensions are added here.
