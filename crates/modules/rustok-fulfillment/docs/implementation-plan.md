# Implementation plan for `rustok-fulfillment`

Last reviewed: 2026-07-31

## Current state

`rustok-fulfillment` owns shipping options, fulfillments, typed fulfillment items,
shipping-selection policy, and provider SPI policy. Commerce composes delivery
groups, checkout, and multi-fulfillment orchestration; it must not duplicate
fulfillment transport, selection materialization, carrier lifecycle persistence,
or fulfillment recovery queries.

The owner storefront handles seller-aware shipping selection through native and
GraphQL transports selected via `execute_selected_transport`. Selection identity is exactly `shipping_profile_slug +
seller_id`; legacy `seller_scope` is not accepted. Provider registry guards
capability, health, unavailable mode, and degraded fallback before an adapter
call, while `FulfillmentService` remains the lifecycle owner.

Checkout fulfillment create/adopt/read enters through
`CheckoutFulfillmentExecutionPort`. Commerce sends typed order-line commands from
the immutable checkout plan and receives normalized owner projections. The owner
uses `FulfillmentService::list_by_order` and `create_fulfillment`; mounted Commerce
checkout no longer queries fulfillment persistence or constructs the service.

The root in-process checkout factory mounts
`TypedCheckoutFulfillmentExecutionPort`. Ensure and recovery reads accept
`Pending`, `Shipped`, and `Delivered`. `Cancelled` and unknown lifecycle values
fail closed with typed manual reconciliation. Durable typed checkout fulfillment
identity and a concurrency-safe uniqueness constraint remain open.

Complete shipping-option active list and lookup use `ShippingOptionReadPort`;
administrative list-all uses the separate `ShippingOptionAdminReadPort`. Root
in-process adapters own `FulfillmentService` construction, require read policy,
preserve requested/default locale, and map owner failures to stable `PortError`.

The application host composes one `CommerceShippingOptionReadRuntime` in
`HostRuntimeContext`. Mounted GraphQL consumes it through manifest runtime data
and resolver scope. Mounted Commerce REST consumes the same runtime through
`CommerceHttpRuntime`.

The following mounted projection reads no longer construct a concrete fulfillment
service or in-process read provider inside Commerce:

- GraphQL validation, enrichment, storefront listing, single lookup, and admin
  list-all;
- REST storefront active-list;
- REST admin list-all and single lookup.

The source transport inventory has status `source_cutover_ready_unvalidated`. It
records host-composed GraphQL/REST source topology, retained filters and HTTP
envelopes, context/deadline propagation, and the absence of concrete read
construction. It explicitly records `runtime_parity_proven: false`.

Fulfillment lifecycle projection lookup, filtered list, and latest-by-order are
published through `FulfillmentReadPort`. `InProcessFulfillmentReadPort` owns
concrete `FulfillmentService` construction, requires read policy, parses tenant
identity from `PortContext`, preserves existing filter and ordering semantics, and
maps every current owner error to stable `PortError`.

Commerce publishes a separate
`CommerceFulfillmentLifecycleReadRuntime`. The default application host reuses an
externally installed runtime or constructs the in-process baseline once, caches it
in `ServerRuntimeContext`, and attaches the same typed value to
`HostRuntimeContext`. `CommerceHttpRuntime` requires that runtime.

The mounted `CommerceShippingOptionReadScope` now carries both shipping-option and
fulfillment-lifecycle runtimes for each GraphQL resolver task. The private
compatibility facade resolves `Arc<dyn FulfillmentReadPort>` from that shared
scope. Fulfillment lookup, filtered list, and latest-by-order now call the three
owner operations while leaving `query.rs` signatures and optional-not-found
behavior unchanged. The previous private concrete `FulfillmentService` field and
constructor are removed.

Admin REST fulfillment list/detail consume the same host-selected owner port. The
cutover preserves page/per-page, status/order/customer filters, owner pagination
total, detail not-found behavior, public HTTP status/code/message policy,
authenticated user actor, request locale/channel, resource correlation, and a
two-second deadline.

GraphQL lifecycle reads preserve the existing lookup `None`, list pagination and
filters, optional latest-by-order projection, compatibility error classes, tenant
identity, stable service actor, resource correlation, and a two-second deadline.
The private compatibility shim now retains the exact typed public GraphQL
message/code/retryable policy for every `PortErrorKind` instead of reducing
forbidden and invariant failures through a dynamic string. Lifecycle mutations
remain on their existing concrete or orchestration owner paths.

A locked mounted projection-parity execution contract and fail-closed capture
runner are now published. The maintainer-owned runner compares GraphQL
lookup/list/latest-by-order with admin REST list/detail, hashes normalized
projections and source files, preserves transport-specific optional-not-found
policy, and excludes credentials, raw bodies, and fulfillment metadata. No capture
has run; `transport_projection_parity_proven` and `runtime_parity_proven` remain
false.

A separate deterministic lifecycle read deadline and typed-failure harness is now
published. It mounts one scripted `FulfillmentReadPort` through the public
GraphQL and HTTP runtime seams, locks the GraphQL/REST error matrices, preserves
optional not-found, records two-second contexts for lookup/list/latest/detail, and
requires owner-message redaction. The harness has not run, so
`deadline_failure_proven` remains false. Restart, external-adapter identity, and
remote adapter execution remain separate evidence gates.

The native FFA surface remains seller/cart selection through
`ShippingSelectionPort`; it does not publish complete projection list or lookup
operations. No projection API should be added without a concrete consumer.

Shipping-selection owner payload diagnostics are source-closed / unvalidated.
Tenant parsing retains only a static failure fact and bounded context shape. All
five `FulfillmentError` variants retain only a static variant plus aggregate
text/UUID/opaque-payload shape. Raw tenant, parser, validation, transition,
resource UUID, and database payloads are not recorded. Read/write admission,
seller/profile filtering, owner delegation, severity, and public `PortError`
envelopes are unchanged. Shipping-option projection and fulfillment lifecycle
read diagnostic payloads remain separate open slices.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `fulfillment.shipping_selection.v1` in
  `crates/rustok-fulfillment/contracts/fulfillment-fba-registry.json`.
- Additional workflow contract: `fulfillment.checkout_execution.v1` in
  `crates/rustok-fulfillment/contracts/fulfillment-checkout-execution-v1.json`.
- Published checkout execution port: `CheckoutFulfillmentExecutionPort`.
- Mounted checkout provider: `TypedCheckoutFulfillmentExecutionPort` over the
  owner execution adapter.
- Source-ready internal read boundaries: `ShippingOptionReadPort`,
  `ShippingOptionAdminReadPort`, and `FulfillmentReadPort`; these are not new FBA
  provider contracts.
- FFA admin guardrail evidence: `scripts/verify/verify-fulfillment-admin-boundary.mjs` locks the fulfillment fast boundary guardrail.
- FFA storefront guardrail evidence: `scripts/verify/verify-fulfillment-storefront-boundary.mjs` locks the fulfillment storefront boundary guardrail.
- Contract/provider evidence remains in the existing fulfillment evidence
  matrices and live-adapter files.
- Shipping-selection diagnostic source evidence:
  `crates/rustok-fulfillment/contracts/evidence/shipping-selection-diagnostic-safety-source.json`.
- Shipping-option source evidence:
  `crates/rustok-fulfillment/contracts/evidence/shipping-option-read-transport-parity-source.json`.
- Fulfillment lifecycle read source evidence:
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-port-source.json`.
- Fulfillment lifecycle mounted capture contract:
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-transport-parity-execution-contract.json`.
- Fulfillment lifecycle deadline/failure contract:
  `crates/rustok-fulfillment/contracts/evidence/fulfillment-lifecycle-read-failure-execution-contract.json`.
- Source evidence and both execution contracts are unvalidated and do not promote
  status.
- Focused guards cover owner boundaries, application-host runtime composition,
  shared GraphQL resolver scope, GraphQL/admin REST lifecycle consumer cutover,
  typed public envelopes, bounded shipping-selection diagnostics, the
  projection-parity capture boundary, the deterministic deadline/failure harness,
  and the separate native selection surface.
- Compile, migrated database, mounted capture, failure-harness execution, restart,
  contention, and remote evidence remain missing.

## Shipping-selection diagnostic source checklist

- [x] Preserve the public `ShippingSelectionPort` trait, request/response DTOs,
  read/write admission order, seller/profile filtering, and owner delegation.
- [x] Replace tenant UUID parser payloads with a static parse-failure fact and
  bounded context shape.
- [x] Replace all five `FulfillmentError` payload diagnostics with a static variant
  plus aggregate text, UUID, and opaque-payload shape.
- [x] Preserve stable public code, message, kind, retryability, and error/warning
  severity.
- [x] Retain focused evidence and broad ecommerce guard coverage without claiming
  compile or runtime validation.
- [ ] Execute the focused verifier, broad verifier/self-test, fulfillment compile,
  native/GraphQL selection parity, restart, and remote-adapter evidence.

## Checkout execution source checklist

- [x] Publish typed create/adopt/read commands through
  `CheckoutFulfillmentExecutionPort`.
- [x] Keep fulfillment persistence and metadata compatibility lookup inside the
  owner module.
- [x] Mount the root in-process factory through typed lifecycle validation.
- [x] Accept pending, shipped, and delivered replay projections.
- [x] Route cancelled and unknown lifecycle states to manual reconciliation.
- [x] Guard mounted Commerce against direct construction of the legacy execution
  adapter.
- [ ] Replace metadata identity with owner-owned typed persistence and a
  concurrency-safe uniqueness constraint.
- [ ] Execute compile, create/adopt/read, duplicate identity, lifecycle,
  process-exit, restart, contention, and remote-profile evidence.

## Shipping-option read source checklist

- [x] Publish active list and lookup through `ShippingOptionReadPort`.
- [x] Publish administrative list-all through
  `ShippingOptionAdminReadPort`.
- [x] Keep storefront active listing and complete admin listing as separate traits.
- [x] Preserve requested and tenant-default locale arguments.
- [x] Require read policy and parse tenant identity from `PortContext`.
- [x] Map all current `FulfillmentError` variants to stable `PortError` values.
- [x] Export canonical root in-process factories.
- [x] Remove direct service construction from mounted GraphQL shipping-option
  validation, enrichment, listing, lookup, and admin list-all.
- [x] Inject both owner ports through application-host composition and resolver
  scope; keep standalone fallback outside the private facade.
- [x] Retain a source inventory that distinguishes complete projection reads from
  seller/cart selection without claiming runtime parity.
- [x] Add the shared runtime to `CommerceHttpRuntime` and cut REST storefront
  active-list plus admin list-all/lookup over to owner ports.
- [x] Preserve storefront currency/channel/profile filters and admin
  inactive-before-filter plus active/currency/provider/search/pagination behavior.
- [x] Preserve existing REST status/code/message policy through typed
  `PortErrorKind` mapping without owner-message control flow.
- [x] Propagate REST tenant, actor, locale, effective channel, correlation, and
  two-second deadline context.
- [ ] Execute compile, mounted GraphQL/REST active-list/list-all/lookup parity,
  deadline, locale, channel, optional-not-found, failure, and remote evidence.

## Fulfillment lifecycle read source checklist

- [x] Publish one owner `FulfillmentReadPort` for single projection, filtered list,
  and latest-by-order reads.
- [x] Preserve the existing `FulfillmentResponse`, list filters, pagination total,
  latest-created ordering, and optional latest-by-order result.
- [x] Require read policy and parse tenant identity from `PortContext`.
- [x] Map every current `FulfillmentError` variant to stable owner `PortError`
  values without owner-message control flow.
- [x] Export the canonical `in_process_fulfillment_read_port` factory.
- [x] Publish `CommerceFulfillmentLifecycleReadRuntime` with public host-selected
  and in-process constructors plus a clone getter.
- [x] Allow Commerce GraphQL runtime-data construction to consume a host-provided
  lifecycle runtime while retaining an explicit in-process compatibility fallback.
- [x] Compose/cache the lifecycle runtime once in the default application host,
  preserve externally installed adapters, and attach it to `HostRuntimeContext`.
- [x] Require the typed lifecycle runtime in `CommerceHttpRuntime` and expose the
  cloned owner port for route handlers.
- [x] Cut admin REST fulfillment list and detail reads over to the owner port while
  preserving page/per-page, status/order/customer filters, owner total, and detail
  not-found behavior.
- [x] Preserve admin REST status/code/message policy through typed `PortErrorKind`
  mapping without owner-message control flow.
- [x] Propagate admin REST tenant, authenticated actor, locale, optional channel,
  resource correlation, and two-second deadline context.
- [x] Keep lifecycle mutation concrete/orchestration service construction
  unchanged.
- [x] Scope the host-selected lifecycle runtime through the existing mounted
  Commerce GraphQL resolver extension.
- [x] Cut GraphQL fulfillment lookup, filtered list, and latest-by-order reads over
  to `FulfillmentReadPort` while preserving lookup `None`, list metadata, and
  optional latest-by-order behavior.
- [x] Preserve GraphQL compatibility error classes through typed `PortErrorKind`
  mapping without owner-message control flow.
- [x] Preserve exact GraphQL public code/retryability for forbidden and invariant
  owner errors through the typed compatibility shim.
- [x] Remove the remaining concrete `FulfillmentService` read field and constructor
  from the private GraphQL compatibility facade.
- [x] Retain source evidence and focused guards that record complete source cutover
  without claiming mounted runtime parity.
- [x] Publish the mounted lifecycle projection-parity execution contract and capture runner.
- [x] Publish the deterministic lifecycle read deadline and typed-failure harness.
- [ ] Execute the mounted GraphQL/REST projection-parity capture and retain its immutable packet.
- [ ] Execute the deterministic deadline/failure harness and retain its immutable result.
- [ ] Prove deadline/failure injection, process restart, and remote-adapter behavior separately.
- [ ] Execute compile and remaining tenant/context/runtime evidence before any
  status promotion.

## Open results

1. **Prove checkout fulfillment identity and replay.** Execute create/adopt/read,
   duplicate key, partial set, concurrent create, process-exit, restart, and
   upgraded metadata scenarios through mounted Commerce.
   **Depends on:** compiled crates and migrated databases.
   **Done when:** one immutable plan produces one exact fulfillment set and every
   conflicting, cancelled, unknown, or duplicate identity fails closed.

2. **Replace metadata identity with typed persistence.** Add owner-owned checkout
   fulfillment identity and uniqueness without a foreign key to Commerce checkout
   tables.
   **Depends on:** upgraded compatibility evidence for current keys.
   **Done when:** recovery no longer scans metadata and concurrent creation cannot
   commit duplicate fulfillment indices.

3. **Prove mixed-cart and multi-fulfillment edge cases.** Cover seller-aware
   selection, partial shipment/delivery, reopen/reship recovery, remaining
   quantity, and grouped checkout interactions.
   **Depends on:** order-line and Commerce delivery-group contracts.
   **Done when:** targeted tests cover valid and rejected transitions for mixed
   carts and multiple fulfillment records.

4. **Wire production carrier adapters through the provider registry.** Add
   production-like quote, label, cancellation, tracking-webhook, and replay-safe
   behavior through guarded provider seams.
   **Depends on:** approved credentials, webhook ingress, and deployment secret
   management.
   **Done when:** execution proves degraded fallback and typed adapter errors while
   `FulfillmentService` remains the lifecycle owner.

5. **Prove mounted shipping-option transport parity.** Execute active list,
   administrative list-all, and lookup through mounted GraphQL and REST consumers
   against the same owner projections. Native seller/cart selection remains a
   separate contract and needs no complete projection surface without a consumer.
   **Depends on:** compiled fulfillment/Commerce/server crates and mounted
   transport fixtures.
   **Done when:** GraphQL and REST retain locale/channel/deadline context, expose
   equivalent projections with correct active/inactive semantics, preserve public
   envelopes, and no mounted projection transport constructs a concrete read
   service or provider.

6. **Prove mounted lifecycle read parity.** Run the locked capture contract against
   GraphQL lookup/list/latest-by-order and admin REST list/detail through the
   host-selected `CommerceFulfillmentLifecycleReadRuntime`. Retain the immutable
   projection-parity packet, execute the deterministic deadline/failure harness,
   then retain restart, external-adapter identity, and remote-adapter evidence as
   separate packets.
   **Depends on:** compiled fulfillment/Commerce/server crates, mounted fixtures,
   and tokens with `fulfillments:read` plus `orders:read`.
   **Done when:** projection parity, tenant/filter/pagination, optional-not-found,
   typed public failures, deadlines, and redaction are retained before wider
   runtime behavior is promoted, without concrete Commerce read construction or
   secret/raw-payload retention.

7. **Execute remote contracts.** Turn shipping-selection and checkout-execution
   matrices into provider execution before promoting beyond `boundary_ready`.
   **Depends on:** a remote adapter environment and a Commerce consumer.
   **Done when:** deadline, idempotency, typed-error, identity, and fallback parity
   are proven.

## Verification

- `npm run verify:fulfillment:admin-boundary`
- `npm run verify:fulfillment:storefront-boundary`
- `node scripts/verify/verify-fulfillment-shipping-selection-diagnostic-safety.mjs`
- `node scripts/verify/verify-ecommerce-public-port-error-safety-v2.mjs`
- `node scripts/verify/verify-ecommerce-public-port-error-safety-v2.test.mjs`
- `node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`
- `node scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs`
- `node scripts/verify/verify-fulfillment-shipping-option-read-port.mjs`
- `node scripts/verify/verify-fulfillment-lifecycle-read-port.mjs`
- `node scripts/verify/verify-commerce-graphql-query-fulfillment-context.mjs`
- `node scripts/verify/verify-fulfillment-lifecycle-transport-parity-capture.mjs`
- `node scripts/verify/verify-fulfillment-lifecycle-read-failure-contract.mjs`
- `node scripts/evidence/capture-fulfillment-lifecycle-transport-parity.mjs`
- `node scripts/verify/verify-commerce-shipping-option-transport-parity-inventory.mjs`
- `node scripts/verify/verify-commerce-admin-shipping-option-error-context.mjs`
- `node scripts/verify/verify-commerce-admin-shipping-http-error-safety.mjs`
- `node scripts/verify/verify-commerce-storefront-auxiliary-http-error-safety.mjs`
- `node scripts/verify/verify-commerce-graphql-shipping-option-typed-error.mjs`
- `node scripts/verify/verify-commerce-graphql-shipping-enrichment-typed-error.mjs`
- `npm run verify:ecommerce:fba`
- `npm run verify:ecommerce:provider-spi-evidence`
- `cargo xtask module validate fulfillment`
- `cargo xtask module test fulfillment`
- `cargo check -p rustok-fulfillment --all-features`
- `cargo check -p rustok-commerce --all-features`
- `cargo test -p rustok-commerce --test fulfillment_read_port_failure_contract -- --nocapture`
- Targeted checkout fulfillment identity/lifecycle/restart/contention tests.
- Targeted shipping-option GraphQL/REST parity, context, error, and remote tests.
- Targeted fulfillment lifecycle owner-port and mounted GraphQL/REST query tests.

No verification or capture command was executed in this source wave.

## Change rules

1. Keep shipping selection, shipping-option projections, fulfillment lifecycle,
   checkout fulfillment identity, and carrier policy in this module.
2. Update local documentation, contracts, `rustok-module.toml`, and the umbrella
   Commerce plan with a delivery/provider contract change.
3. Update status blocks and `docs/modules/registry.md` only with proven FFA/FBA
   boundary changes.
