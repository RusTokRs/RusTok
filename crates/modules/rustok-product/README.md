# rustok-product

> Canonical large-module reference hardening is in progress.
> See [docs/reference-hardening-current.md](docs/reference-hardening-current.md) for the current
> Blog-v1 parity cursor. Existing Product capability/evidence documents remain authoritative for
> their own runtime and feature scopes.
> See [CRATE_API.md](CRATE_API.md) for the verified public crate API specification and platform capability bindings.

## Purpose

`rustok-product` is the default catalog submodule of the `Ecommerce` family.

## Standalone catalog-read profile

Product remains one domain module. Its public ports, DTOs and local implementation
currently live in `rustok-product`. The existing infrastructure packages remain separate:

- `rustok-product-catalog-service`: authenticated standalone provider for three catalog reads.
- `rustok-product-transport`: tonic client/server framing and bearer authentication.

Keep these existing transport/service packages separate under repository policy. Their
existence supports a bounded remote capability; it does not make the whole Product module
remotely interchangeable. See [FBA scope](../../../docs/backend/module-backend-architecture.md#fba-contracts).

## Known Limitations / Pending Implementation

The current FBA remote profile is a **three-operation catalog-read slice**, not a
whole-module Product deployment. `RUSTOK_PRODUCT_CATALOG_PROVIDER=grpc` installs
`ProductCatalogReadRuntime::external(...)`. Default command composition separately
constructs the embedded `ProductCatalogCommandRuntime`.

| Surface | Current selected remote profile |
|---|---|
| Product projection, variant-to-product projection, basic published list | Implemented by the three gRPC RPCs. |
| Filtered storefront list, facets and storefront detail | Extra read-port methods are not implemented by the gRPC client and return typed unavailable. |
| Variant identity, schema, HTTP and tag read handles | Not supplied by the default external read runtime; consumers needing them must fail explicitly. |
| Native admin list/facets and schema operations | Some paths still construct embedded owner services. |
| Product commands and Product/Pricing/Inventory transaction collaboration | Embedded under current default composition; no remote command provider is selected by this setting. |

The July 29 maintainer attestation records historical harness results, without an exact
source commit or retained raw logs. A November 6 source review found that the transport
conformance fixture's `ProductResponse` literal omits the now-required `brands` field;
current compilation/conformance must be repaired and rerun before claiming a current pass.
This documentation review did not execute Cargo or change that test.

Separate-process provider startup and authenticated mounted Commerce/AI business evidence
remain open. Existing `boundary_ready` status is bounded to the catalog-read track.
Product does not currently publish a contracts-only build without ORM dependencies.
Contracts and implementation may remain in this one owner crate. See the
[coverage and verification work](docs/implementation-plan.md#fba-deployment-scope-review-2026-11-06).


## Responsibilities

- Exact Product-owned image references with retain-before-commit, durable postcommit
  release delivery, privileged legacy backfill and diagnostic-only reconciliation.
  [Accepted workflow / open execution gates](docs/media-reference-retention.md).

- Product entities, translations, variant axes, variants, and product-owned migrations.
- PostgreSQL target-schema enforcement removes unused compatibility columns,
  requires Media-owned image identifiers, preserves decimal variant weights,
  and maintains an indexed globally visible storefront page path.
- One Product-owned public-error policy redacts internal database details across
  GraphQL and native admin/storefront transports while preserving stable codes
  and correlation references.
- Native catalog categories, category-bound product forms, reusable product
  attribute schemas, product attribute dictionaries, typed product/variant
  attribute values, and highload-ready category/value projections.
- Typed product attribute value reads and transactional patches validate the
  effective category schema and option ownership, keep localized text in the
  requested translation row, preserve detached values, and publish one outbox
  event per patch request.
- Product publication validates required effective attributes before a product
  becomes active; localized text-like required values need an explicit non-empty
  translation row, option attributes need stored option relations, and
  create-with-publish is rejected when required typed attributes cannot yet be
  populated.
- Detached values remain visible to product operators after a primary-category
  change and can be explicitly cleared through owner-owned native server
  functions with parallel GraphQL; the service rejects attempts to clear values
  that are still effective.
- Effective product forms expose localized option dictionaries and localized
  group labels with bounded reads; schema/category group creation and
  `group_code` bindings are available over native server functions plus
  parallel GraphQL, and the module-owned admin renders grouped typed editors
  with dirty-field patch semantics.
- Own positive monotonic `index_revision` storage columns for Product and
  ProductVariant. Product updates and Product-translation changes advance the
  Product revision; every ProductVariant update advances the variant revision.
  Variant insert/delete/move membership also advances the affected parent
  Product revision. These are owner-input watermarks, not the complete Product
  Index mutation clock.
- Own retained hard-delete replay state in `product_index_tombstones` and
  `product_variant_index_tombstones`. Physical Product deletion retains every
  current translation locale, direct translation deletion retains the exact
  removed locale, and ProductVariant deletion retains its non-localized key.
  Recreated identities receive an `index_revision` strictly above the retained
  delete before the tombstone is cleared.
- Own append-only `product_sales_channel_index_relation_snapshots` with a
  dedicated monotonic relation epoch, bounded current/change readers,
  idempotent complete-membership replacement, live-Product delete fencing, and
  retained empty membership after Product hard delete. Product still does not
  resolve Channel slugs or depend on `rustok-channel`/`rustok-index`.
- Own append-only `product_index_graph_projection_snapshots`, whose
  `projection_epoch` advances when either retained Product state or resolved
  SalesChannel membership advances. This is the complete Product graph mutation
  clock used by the selected Index source.
- Publish only a neutral `ProductRuntimeSelected` marker for selected
  cross-module composition. The Product crate does not depend on `rustok-index`
  and does not construct generic Index mutations.
- The selected `rustok-distribution` bridge publishes exactly one current
  Product Index contract and one current ProductVariant contract. Product replay
  is locale-aware and enumerates stable `(product_id, locale)` identities;
  ProductVariant replay is non-localized and enumerates stable `variant_id`
  identities.
- The current Product Index graph carries Product identity/scalars,
  `variant_ids` with a many `variants` link, and `sales_channel_ids` with a many
  `sales_channels` link. Product visibility slugs remain Product-owned resolver
  input and are not duplicated as transitional Index fields.
- Product replay uses `projection_epoch` as the full-record mutation
  `source_version`; ProductVariant replay uses its owner `index_revision`.
  Product locale absence uses the same Product projection clock and fails closed
  until the projection matches the current Product watermark.
- The selected distribution composition resolves Product channel visibility to
  current tenant Channel UUID membership and writes only through the
  Product-owned relation store. Unrestricted Product visibility resolves
  against the current tenant Channel identity universe; Channel runtime active
  state remains Channel-owned. This resolver is bounded and convergent, not an
  atomic cross-owner snapshot or continuous event consumer.
- The selected bridge emits `IndexMutation::Upsert` for live owner rows and
  `IndexMutation::Delete` for retained hard-delete identities. The canonical
  Product graph has no parallel Product compatibility source/schema branches.
- The Index-owned bounded multi-pass reconciliation runner can restart these
  sources from the beginning and catch live or retained identities inserted
  behind an earlier cursor. It is explicit and crash-resumable, but it does not
  establish a repeatable-read owner snapshot or close the final-pass
  concurrent-write window. Durable relation convergence triggering, retained
  freshness/equivalence evidence, event-contract admission, tombstone
  retention/purge admission, and authoritative Index cutover remain open.
- Effective visibility is resolved as tri-state overrides with precedence
  `attribute defaults < schema/category overrides < channel settings`.
- Virtual categories use a validated, bounded V1 rule contract over product
  status, primary-category subtree, intersecting price range, stock state, and
  effective locale-neutral product attribute equality/ranges. Materialized
  assignments remain reserved storage until an owner-owned runtime is implemented.
- Catalog category writes preserve a canonical closure projection; deferred
  PostgreSQL constraints reject parent cycles and closure drift at commit.
- Product-owned relation storage for taxonomy-backed tags (`product_tags`).
- Product write-side services and publication lifecycle.
- Responsibility-specific catalog components: commands, queries, product
  projection, tags, typed values, and effective-form resolution remain behind
  the stable `CatalogService` and `ProductCatalogSchemaService` entry points.
- Product-side synchronization of first-class `tags` contract fields with the
  taxonomy-backed dictionary.
- Product-side normalization of first-class `shipping_profile_slug` onto the
  temporary metadata-backed shipping profile contract, without erasing an
  existing metadata-backed profile when the typed field is omitted.
- Product-side ownership of nullable `seller_id` as the canonical marketplace
  identity key that downstream cart/order/fulfillment flows consume; merchandising
  fields such as `vendor` remain display-only and are not used as seller identity.
- Product-side split and locale-aware resolution of Flex attached custom-field
  values, using shared `flex` attached localized storage while preserving
  non-Flex operational metadata in `products.metadata`.
- Publish a module-owned Leptos admin UI package in `admin/` for catalog CRUD,
  publication lifecycle, and shipping-profile selection.
- Publish a module-owned Leptos storefront UI package in `storefront/` for
  published catalog discovery, handle-based product selection, and
  channel-aware inventory visibility.
- Keep generic catalog price snapshots available for product-owned CRUD and
  discovery flows through the decoupled consumer-owned `ProductPricingPort`
  contract, while treating pricing-authoritative reads as the
  responsibility of `rustok-pricing` surfaces (`adminPricingProduct` /
  `storefrontPricingProduct`).
- Keep product-owned admin/storefront UI aligned with that split by rendering
  catalog snapshot pricing separately from pricing-module previews instead of
  using generic `variants.prices` as resolved pricing.
- Keep storefront shell copy, typed fetch request shape, selected-card labels,
  empty state, and rail presentation state in the framework-agnostic storefront
  core so Leptos remains a host-context/render adapter over native + GraphQL
  transport parity.
- Publish the owner `ProductCatalogReadPort` / `product.catalog_read.v1`
  boundary for catalog-read consumers. The in-process `CatalogService`
  implementation has live PostgreSQL execution evidence. The composed
  `rustok-ai` consumer has live unavailable/deadline degraded-path evidence;
  Commerce checkout currently treats Product as a hard dependency. The external adapter exists for three reads. The module
  remains `boundary_ready` until current-revision conformance and the declared
  separate-process consumer/business evidence are retained; Commerce has no
  implicit snapshot or local-provider fallback for a failed selected Product read.
- Product module metadata for runtime registration.
- Product-owned catalog search metadata for optional category filters and
  filterable/sortable attribute controls in admin/storefront search UI. Hosts
  inject those options through composition; search UI does not import product
  internals or negotiate locale itself.
- Product translation title search predicates are not owned by `apps/server`;
  shared ecommerce readers use the owner/foundation search helper instead of a
  host-local `product_search` service.
- `StorefrontProductListQuery` owns storefront filters, sorting, and validated
  page/per-page input as one Product query contract across native and GraphQL
  transports.

## Interactions

- Owns Product DTOs, ORM entities, errors, tables, and migration history without
  a `rustok-commerce-foundation` or `rustok-index` dependency.
- Uses `ProductPricingPort` for decoupled pricing projections and
  atomic price lifecycle operations without direct persistence coupling.
- Depends on `rustok-inventory` for inventory-owned bootstrap and availability
  operations.
- Depends on `flex` for shared attached localized-value storage helpers used by
  product custom-field multilingual flows.
- Depends on `rustok-taxonomy` for shared scope-aware tag dictionary while keeping `product_tags`
  module-owned.
- Depends on `rustok-outbox` and `rustok-events` for transactional event publishing.
- Is consumed by the selected distribution bridge for generic Index schema/source
  composition and Product-to-Channel relation resolution; Index core and server
  replay composition remain Product-agnostic.
- Used by `rustok-commerce` as the umbrella/root module of the ecommerce family.
- Consumed by `apps/admin` through manifest-driven module UI composition.
- Consumed by `apps/storefront` through manifest-driven module UI composition.
- Consumed by `rustok-search` UI hosts through product-owned catalog search
  metadata helpers for category filters and attribute filter/sort controls.
- Consumed through `ProductCatalogReadPort` by Commerce checkout and the
  `rustok-ai-product` support adapter composed by `rustok-ai`. Pricing captures selected Product read/identity capabilities and explicitly
  refuses missing identity or unsupported external SKU authoring. Its native
  transaction collaboration remains separate from the three-read remote profile.

## Fulfillment capability gap

The catalog already admits `Digital` as a product-type value, but the current
shipping-profile path does not yet carry one canonical typed fact that proves whether
a product/variant requires physical fulfillment. The target must make that requirement
explicit: digital products carry no shipping requirement or synthetic default profile;
physical products carry the typed requirement and compatible profile snapshot consumed
by Cart and Order. Fulfillment is therefore conditional on domain data, not a Product
module dependency. See
[`docs/architecture/settings.md`](../../../docs/architecture/settings.md).

## Entry points

- `ProductModule`
- `ProductRuntimeSelected`
- `CatalogService`
- `ProductCatalogReadPort`
- `ProductSalesChannelIndexRelationStore`
- `services::catalog_schema::resolve_effective_product_form`
- `ProductCatalogSchemaService`
- `admin::ProductAdmin`
- `storefront::ProductView`

See also `docs/README.md`, the Index
[M7 canonical Product graph contract](../rustok-index/docs/m7-product-graph-source.md),
[M7 Product-SalesChannel resolver contract](../rustok-index/docs/m7-product-sales-channel-resolver.md),
[M7 Product tombstone replay contract](../rustok-index/docs/m7-product-tombstone-source.md),
[M7 Product graph projection ledger](docs/index-graph-projection-ledger.md), and
[M7 bounded Product reconciliation contract](../rustok-index/docs/m7-product-reconciliation.md).

The canonical architecture for variant identity and configuration is the
[Unified variant axis architecture](../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md).
Variants are defined along configured product axes (`product_variant_axes` and `product_variant_axis_values`),
with combination identity (`combination_identity`) and completeness invariants maintained and verified at the
database layer. Current writers use the unified model; that source fact does not certify the absence
of historical tables or externally consumed data in an installation.

## Native storefront context admission (2026-10-18)

Catalog list, facets, detail/Pricing and search options now require the canonical trusted
`RequestContext` before owner reads. Extractor rejection is no longer treated as an absent
optional context; valid anonymous/no-channel contexts remain allowed. Raw rejection/channel/
locale/correlation payload is removed from affected context diagnostics. See the
[current contract](docs/product-public-error-diagnostic-safety.md#native-storefront-context-admission-continuation-2026-10-18).
This is source-only, unintegrated work: five new negative guard scenarios are unrun;
compilation, host middleware composition and mounted behavior remain maintainer gates.

## Schema/tag read diagnostic continuation (2026-10-18)

Both owner read adapters now reuse bounded Product context/error fact projections instead
of logging raw errors, admission message/code text or identity values. Existing classification,
public envelopes and read admission are preserved. See the [current contract](docs/product-public-error-diagnostic-safety.md#schema-and-tag-read-diagnostics-continuation-2026-10-18).
Eight scoped source guards and four-file formatting/idempotence pass; three new Rust scenarios
are unrun. Compilation, log capture, runtime and broader Product diagnostics remain open.

## Storefront attribute warning safety (2026-10-18)

Unsupported-type/missing-scalar projection warnings now retain bounded shape only, without
stored identity/code/type/error payload. Existing best-effort omission and valid false/zero/
localized formatting semantics are preserved. See the [current contract](docs/product-public-error-diagnostic-safety.md#storefront-attribute-degradation-diagnostics-continuation-2026-10-18).
Six scoped source guards and one-file formatting/idempotence pass; four new Rust scenarios
are unrun. Compilation, captured tracing and mounted projection execution remain open.

## Storefront HTTP read warning safety (2026-10-18)

The compatibility read port now logs bounded context/error shape and pagination failure flags,
not caller identity strings or exact rejected page inputs. Existing HTTP classification,
visibility and pagination behavior are preserved. See the [current contract](docs/product-public-error-diagnostic-safety.md#storefront-http-read-diagnostics-continuation-2026-10-18).
Six scoped source guards and one-file formatting/idempotence pass; two added Rust scenarios
are unrun. Compilation, captured tracing and mounted HTTP execution remain open.

## Catalog command diagnostic safety (2026-10-19)

Seven Product command-boundary events now use canonical context/error facts instead
of caller correlation/tenant strings, Product UUIDs and raw receipt/admission codes.
Public mapping, write admission, request fingerprints and receipt replay/finalization
are preserved. See the [current contract](docs/product-public-error-diagnostic-safety.md#catalog-command-diagnostics-continuation-2026-10-19).
Six scoped source guards and Rust formatting/idempotence pass; two new Rust scenarios
are deliberately unrun. Compilation, tracing capture and transaction/runtime gates remain open.

## Known Limitations / Pending Implementation

The [native catalog deletion prerequisite](docs/product-lifecycle-collaboration.md#catalog-deletion-retention-2026-11-01-source-only)
now refuses any Inventory reservation/shipment/return history and individually nonzero stock
counters through both real Product callers. Positive tenant/item proof, bounded complete plans,
shared locks, exact conditional effects and rollback are implemented in source; 31 Rust/10 Node
definitions remain uncompiled/unrun. Migration I026 only adds history-reference indexes and is
unexecuted. Finite native/HTTP/GraphQL policy is synchronized, but actual Next/Leptos/transport
execution and full actionable recovery parity are unverified. Leptos remaining CRUD still uses
the generic GraphQL gateway/redaction and non-durable retained CRUD keys. This is not a general
SKU-retirement/disposal workflow, Inventory Index tombstone or Product step-8 certification.

The [variant-axis continuation](docs/implementation-plan.md#variant-axis-continuation-2026-10-29)
is **in progress**, not a certified Product cutover. Source now preserves binding policy on
rebind, validates eligibility/defaults and effective inherited/clone policy, refuses schema
changes that invalidate existing Products under a transaction fence, and creates axes plus
selected sparse SKU tuples atomically through the existing create command. Next and the routed
Leptos editor carry these inputs; standalone SKU creation carries assignments.

No tests were run for this continuation, as requested by the maintainer. Rust compilation,
PostgreSQL concurrency and mounted-client execution remain unverified; expanded Next UI/host
checking is blocked by the incomplete dependency graph. Only scoped TypeScript core checking,
source formatting, JavaScript syntax and whitespace checks passed. The architecture gate still
requires unavailable `cargo metadata`; ADR verification also reports existing registry/metadata/heading inconsistencies outside the unified-axis ADR.

Existing-axis preview/reassignment/CAS and bounded sparse combination picking have source
implementations linked below, not runtime sign-off. Attribute/option update/archive/restore now
also has a selected-owner, receipt-bound source implementation with both admin clients. Storefront
tuple selectors and cross-owner Pricing/Inventory/Channel/Index/Cart/Order/Media/Fulfillment
evidence remain unfinished. Taxonomy reparent concurrency and realistic tenant
snapshot/lock cost need verification. Axes and standalone SKU creation now have shared durable
Leptos recovery in source, with Next parity. Other in-memory retained lifecycle keys remain
non-durable; Product create and remaining CRUD still use the GraphQL lifecycle gateway. Native
standalone creation is not a general native CRUD cutover. Remaining initial/schema copy
localization and runtime recovery verification stay open.
The mounted Leptos SKU section intentionally has no destructive retirement action. The source-only
native deletion guard is not a general retirement workflow or permission to enable that UI.

Only source consumers were inventoried. Installed rows, FKs, migration application history and
external callers are unknown. No canonical model/table or historical migration was removed or
rewritten. Active attribute options, compatibility snapshots, replay journals and owner links
must not be retired based on their names. See the plan for the retained-source inventory,
explicit remaining work and maintainer checks.

## Existing-axis reconfiguration

The selected `ProductCatalogCommandPort` provides a read-only bounded preview and the existing
receipt-bound `set_variant_axes` applies a proposal-specific CAS with optional full SKU tuple
reassignments. Variant/configuration UUIDs remain stable and no SKU/stock/removed-axis EAV is
silently deleted. Next and Leptos include explicit review and immutable axes-request recovery
in source. This is not a completed storefront/retirement cutover or runtime certification:
see the [owner contract and release gates](docs/variant-axis-reconfiguration.md).

## Bounded sparse combination picking

Both admin clients now browse configured allowed-value combinations in 12-row pages, refusing
spaces above 4096 and invalid/oversized membership instead of claiming availability. Local
filters do not alter axes; one explicit pick adds one local initial row or fills only the saved
SKU form's full tuple. Saved SKU lists display actual assignments. No Cartesian SKU batch,
inferred price or stock movement is created. Blank price is absent, while explicit zero remains
a price. The canonical owner still validates and writes through the existing commands.
See the [workflow, bounds and unexecuted gates](docs/variant-combinations.md). Source implementation
is not Rust/mounted-client certification; no tests were run.

## Standalone SKU recovery

Both admin clients freeze complete input, caller key, original locale and authenticated scope
before IO, recover without Product GET and retain terminal input for explicit correction. Next
extends the existing Product journal; Leptos shares the axes journal/browser adapter and selects
native JSON or GraphQL before IO. Unknown outcomes never rotate or retarget the saved request;
a Variant UUID is not a new Product revision. No receipt or historical migration changed.
See the [bounded owner contract and unexecuted gates](docs/variant-axis-reconfiguration.md#standalone-sku-creation-and-recovery).
This is source implementation, not runtime or cross-owner certification; no tests were run.

## Attribute and option lifecycle

The existing schema read/write ports now expose bounded archived-inclusive definition reads and
one `update_attribute_definition` command for presentation update/archive/restore. Stable IDs,
codes/storage shape and SKU/EAV/Pricing/Inventory links are not replaced or deleted. Archive
rechecks current bounded references under the schema fence and records typed refusals; restore
does not cascade. Exact-copy semantics, deferred default hints, bounds, locks and the non-monotonic
state fingerprint are specified in [ADR §10c](../../../DECISIONS/2026-09-18-unified-variant-axis-architecture.md#10c-attribute-and-option-definition-lifecycle).

Next and mounted Leptos use `attribute_id`/`option_id`, dedicated scopes in their existing caller
journals, exact unknown replay independent of dictionary GET, and terminal-only explicit Fresh.
Missing IDs remain visible/uneditable, not replaced by the parent/first option. Native and GraphQL
select the same installed owner capability before IO. Regression and ignored PostgreSQL sources
are added but **no tests were run**; Rust compilation, SQL, mounted clients and installed-state
inventory remain unverified. This does not finish general CRUD durability or storefront/downstream
cutover. See the [write boundary](docs/write-boundary.md#attribute-and-option-definition-lifecycle).

## Pricing identity collaboration (2026-10-29)

`ProductVariantIdentityReadPort` provides tenant-scoped Variant-to-Product UUIDs for
Pricing, without loading merchandising/attribute/media/inventory projections per
SKU. `read_variant_identities` accepts 1–256 unique non-nil SKU IDs and optional expected
Product membership, preserving request order or refusing the entire read.
`list_product_variant_identities` accepts an already selected page of at most 48
Products and refuses more than 256 SKUs per Product. Only identity columns are read.
This is not a public discovery API, visibility bypass or SKU reservation; public
Product visibility and cart/checkout revalidation remain their existing owners.

Pricing's bounded batch/snapshot/source contract is in its
[crate API](../rustok-pricing/CRATE_API.md). The next Product storefront block still
must connect real sparse tuples, stable SKU/quantity URLs, complete context validation,
live quote refresh and cart-context handoff in both clients. Current source/type
checks do not certify that unfinished buyer flow or runtime concurrency.

## Canonical quantity writer prerequisite (2026-11-01; source only)

Native set/adjust and Product's real quantity-update hook now delegate to one Inventory-owned
bounded plan: current tenant/actor/SKU, saved positive identities, complete 4096 levels, original
location/current binding proof, checked signed arithmetic and exact effects. Quantity is global
available stock; only the existing active owner-default level's stocked counter changes.
Reservations/incoming/history/metadata/other locations are preserved. Missing state refuses;
no-op stock plans are read-only. Existing v1 stock notifications are preflighted and canonically
published on the same transaction; no negative snapshot is clamped or wire-broadened.

See the [source packet](../rustok-inventory/contracts/evidence/quantity-writes-source.json) and [owner contract](../rustok-inventory/CRATE_API.md#canonical-quantity-writes-2026-11-01-source-continuation).

### Known Limitations / Pending Implementation

33 Rust/10 Node focused definitions remain uncompiled/unrun; direct source checks are not SQL,
concurrency, client or runtime evidence. Native stock commands remain non-idempotent and current
Inventory client redaction is unchanged. No baseline, revision/tombstone, generic Index, full
Next/Leptos recovery parity or Product step-8 readiness is claimed.

## Inventory source-history continuation (2026-11-01; source only)

Native Product fresh bootstrap, quantity and exact guarded deletion now participate in the
[Inventory-owned baseline/revision journal](../rustok-inventory/CRATE_API.md#inventory-source-history-2026-11-01-source-continuation).
The bootstrap helper requires the actual caller transaction. A building baseline rejects actual
stock/identity effects, while stock no-op remains read-only; new source provenance failures use
conservative nonterminal storage/infrastructure envelopes, not terminal Product validation.
Existing Product receipt/rollback/publication stays authoritative; no Product revision substitutes
for an Inventory version and no Inventory-triggered Product refresh is added.

### Known Limitations / Pending Implementation

18 Rust/8 Node new source-history definitions are uncompiled/unrun. Baseline activation is an
explicit creator-actor-bound native Inventory API, with no operator HTTP/GraphQL/Next/Leptos
cutover, automatic reset or actor reconstruction. Canonical publication/relay was pending at
this handoff; the continuation below adds that source slice. The selected consumer source continuation below supersedes the missing Distribution/Index
source/event/worker code boundary; recovery/runtime gates remain open. No step-8 readiness is promoted.

## Inventory canonical source delivery (2026-11-02; source only)

Fresh Product/Variant bootstrap and both guarded deletion paths now forward the current actor
explicitly. Inventory writes its own retained change UUID/positive revision or tombstone fact
on the same caller transaction; Product refresh revisions/causation remain independent. Final
baseline activation is canonical, but staged pages never announce ready SKUs. Required Inventory
module handling refuses unmatched broker-only success; Product remote-only routing is preserved.
[Owner evidence](../rustok-inventory/contracts/evidence/source-delivery-source.json) lists 26 new
Rust/10 Node definitions, all uncompiled/unrun. The selected-consumer continuation below now authors current-authority source/initial enqueue/
worker composition. Full operator-client recovery/runtime verification remain open; no step-8 promotion.
Historical migrations and release digest remain unchanged. No live baseline before real handlers
and operator preflight/recovery; missing older facts are not fabricated from status/journal.

## Selected Inventory Index consumer (2026-11-02; source only)

The [selected Distribution bridge](../rustok-distribution/docs/inventory-index.md) now authors
`inventory-stock-primary` / `rustok-inventory/stock`, real sealed Changed/activation listeners,
current authenticated native reads, durable generic persistence before Changed success and
original-baseline-UUID replay enqueue before activation success. The canonical Index runtime
registers bounded generic ModuleWork replay with exact job/checkpoint fences; no event actor,
baseline creator, receipt, journal or saved claim grants source authority. Old redelivery keeps
the currently loaded owner's immutable mutation UUID. Product/other remote consumers remain.
[Current source evidence](../rustok-inventory/contracts/evidence/source-index-consumer-source.json)
supersedes only the missing-consumer-code boundary of preceding source handoffs.

### Known Limitations / Pending Implementation

The runner still parks page failures terminally. Automatic failed-job backoff and authorized
original-job recovery remain unwired; crash/lease expiry and successful bounded yield resume
without UUID/checkpoint replacement. Lease attempts are not failure-only counters. Real credential/
policy/secret rotation, schema/digest/backend/concurrency/broker/restart/deployment evidence and
full native/HTTP/GraphQL/Next/Leptos operator recovery remain pending. No historical delivery is
fabricated; no live baseline/cutover before preflight/recovery. Tests/build/SQL/migrations/runtime
were not executed. Source/worker presence or initial enqueue is not Index or Product step-8 readiness.
