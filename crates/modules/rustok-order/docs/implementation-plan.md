# Implementation plan for `rustok-order`

Last reviewed: 2026-07-31

## Current state

`rustok-order` owns order lifecycle, snapshots, adjustments, tax lines,
transactional outbox events, returns storage, and the module-owned admin and
storefront packages. The server composes owner-owned dashboard analytics only;
it does not query order events directly. `rustok-commerce` provides checkout
orchestration, not an order-service facade or duplicate order transport.

The post-order foundation includes returns and item validation plus a preview /
apply / cancel order-change skeleton. It deliberately does not perform payment
or fulfillment side effects. Checkout completion is owner-owned through
`CheckoutCompletionPort`; the public GraphQL and native storefront paths use
the same typed request/result contract and select transport via `execute_selected_transport`.

Order owns `order_checkout_identities`, an immutable typed projection that binds
a checkout operation to one order and, for live writes, one source cart plus
immutable snapshot/request hashes. `CheckoutOrderIdentityPort` publishes reads,
bind, and explicit legacy adoption over that owner state. `CheckoutCompletionPort`
now owns idempotent create/place/replay plus result reads by cart and operation.

Staged checkout invokes the completion command instead of constructing
`OrderService` or separate creation/confirmation executors. An explicit
order-owned in-process recovery adapter supplies the full order-line projection
needed for inventory adoption and accepts the previous staged hash format only
for upgraded/crash recovery. New order creation never uses that compatibility
adapter.

The recovery adapter and mounted commerce projection validation now use the
canonical `OrderStatusKind`. Pending legacy orders resume through the owner
confirm command. Confirmed, paid, shipped, and delivered outcomes are replay-safe;
cancelled and unknown lifecycle states fail closed. Persisted and transport
status fields remain strings for compatibility.

Checkout compensation invokes `CheckoutOrderCompensationPort`. Identity
resolution, legacy adoption, lifecycle reads, cancellation, replay adoption, and
safe error mapping remain inside `rustok-order`. Commerce receives only a typed
nullable compensation snapshot and no longer constructs `OrderService` on the
mounted compensation path.

The currently identified checkout compensation wrapper and owner
payload-diagnostic sites are source-closed / unvalidated. Public wrapper events
retain only stable `PortError` kind and message shape. Owner events retain only
static `OrderError` variant, aggregate text/UUID/opaque-payload shape, static
parse failure facts, and transition/reconciliation text shape. Public envelopes,
identity policy, cancellation, replay, and lifecycle routing are unchanged.

Captured checkout payment settlement invokes
`CheckoutOrderPaymentSettlementPort`. Order owner validates checkout, cart, order,
and payment-collection identity; transitions a confirmed order to paid; and adopts
paid, shipped, or delivered replay only when the payment reference and method
match. The mounted commerce fulfillment stage no longer constructs `OrderService`.

The currently identified checkout payment-settlement post-delegation mapper and
canonical owner payload-diagnostic sites are source-closed / unvalidated. Wrapper
events retain only stable `PortError` kind and message shape. Owner events retain
only static `OrderError` variant, aggregate text/UUID/opaque-payload shape, static
parse-failure facts, and a closed lifecycle status label. Public envelopes,
identity policy, settlement, replay, and payment-identity routing are unchanged.

The shared checkout write-admission and context-rejection diagnostics used by both
compensation and settlement are source-closed / unvalidated. Admission events
retain stable code, static `PortErrorKind`, message presence/length, retryability,
and safe context shape. Tenant, actor, and causation rejections retain only static
parse-failure, expected-operation presence/non-nil, and mismatch facts plus the same
bounded error shape. Complete `PortError`, message text, debug kind, and UUID parser
payloads are not retained. Admission severity and tenant/actor/causation ordering
are unchanged.

Legacy order metadata remains a temporary compatibility input only inside
order-owned adapters. Legacy rows retain `NULL` for unknown cart, payment,
shipping, or hash facts rather than fabricating attribution. The metadata bridge
and old JSON indexes must be removed after all completion/result consumers use
typed identity exclusively.

Complete order, return, and order-change projection reads are published through
`OrderReadPort`. `CommerceOrderReadRuntime` carries one host-selected owner port
through the default server, `HostRuntimeContext`, Commerce HTTP, and Commerce
GraphQL schema data. Mounted GraphQL, storefront HTTP, and admin REST complete-order
and post-order query consumers use the same port with authenticated actor, resolved
channel, effective locale context, deadline, public error policy, filters,
ordering, pagination total, locale fallback where applicable, and ownership
behavior preserved. Runtime evidence remains open and unvalidated. The superseded
admin post-order GET functions remain compiled but unmounted until maintainer
compile and mounted-parity validation permits their removal.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `order.checkout_completion.v1` in
  `crates/rustok-order/contracts/order-fba-registry.json`.
- Additional workflow contracts:
  - `order.checkout_compensation.v1` in
    `crates/rustok-order/contracts/order-checkout-compensation-v1.json`.
  - `order.checkout_payment_settlement.v1` in
    `crates/rustok-order/contracts/order-checkout-payment-settlement-v1.json`.
- Published provider ports: `CheckoutCompletionPort`,
  `CheckoutOrderIdentityPort`, `CheckoutOrderCompensationPort`, and
  `CheckoutOrderPaymentSettlementPort`.
- Source-ready internal projection boundary: `OrderReadPort`; it is not a new FBA
  provider contract.
- Order read source evidence:
  `crates/rustok-order/contracts/evidence/order-read-port-source.json`.
- Static contract evidence:
  `crates/rustok-order/contracts/evidence/order-contract-test-static-matrix.json`.
- `scripts/verify/verify-order-admin-boundary.mjs`,
  `scripts/verify/verify-order-storefront-boundary.mjs`,
  `scripts/verify/verify-order-read-port.mjs`,
  `scripts/verify/verify-commerce-graphql-order-read-shim.mjs`,
  `scripts/verify/verify-commerce-storefront-order-read-cutover.mjs`,
  `scripts/verify/verify-commerce-storefront-post-order-read-cutover.mjs`,
  `scripts/verify/verify-commerce-admin-post-order-read-cutover.mjs`,
  `scripts/verify/verify-commerce-admin-order-route-error-context.mjs`,
  `scripts/verify/verify-commerce-storefront-transport-handoff.mjs`,
  `scripts/verify/verify-commerce-order-identity-boundary.mjs`,
  `scripts/verify/verify-commerce-checkout-completion-cutover.mjs`,
  `scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs`,
  `scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`, and
  `scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs` lock the current
  UI, transport, projection, identity, staged-consumer, lifecycle, compensation,
  and payment settlement split.
- No status promotion is allowed from source inspection. Clean/upgraded
  migrations, compile/tests, contention, restart, and remote-profile evidence
  remain missing.

## Checkout identity, completion, compensation, and settlement workstream

- [x] Create owner-owned `order_checkout_identities` persistence without a
  foreign key to commerce-owned checkout tables.
- [x] Enforce one identity per checkout operation, order, and known source cart.
- [x] Enforce tenant/order consistency and monotonic identity enrichment on
  PostgreSQL, SQLite, and MySQL source paths.
- [x] Backfill valid operation and hash facts from legacy metadata without
  inventing an unknown source cart or missing hashes.
- [x] Publish typed reads by operation/cart plus idempotent bind and explicit
  legacy adoption through `CheckoutOrderIdentityPort`.
- [x] Keep legacy metadata lookup inside order-owned compatibility adapters;
  consumers receive typed snapshots/projections and safe `PortError` values.
- [x] Implement one idempotent `CheckoutCompletionPort` owner command for
  create/place/replay and result reads by cart/operation.
- [x] Retain payment collection and shipping-option facts as monotonic typed
  identity fields when those facts are available.
- [x] Cut staged checkout creation/confirmation over to
  `CheckoutCompletionPort` and owner-provided typed recovery projection.
- [x] Use `OrderStatusKind` in checkout recovery and mounted projection
  validation; unknown states fail closed without raw string policy matching.
- [x] Publish `CheckoutOrderCompensationPort` with operation/cart/order identity,
  owner-local legacy adoption, cancellation replay, and manual-reconciliation
  outcomes for orders with financial or fulfillment effects.
- [x] Cut mounted checkout compensation over to the order compensation port.
- [x] Close the currently identified checkout compensation wrapper and owner
  payload-diagnostic sites at source level without changing public envelopes,
  identity policy, cancellation, replay, or lifecycle behavior.
- [x] Publish `CheckoutOrderPaymentSettlementPort` with typed checkout/payment
  identity, owner-local settlement, replay adoption, and payment-reference
  conflict classification.
- [x] Cut mounted checkout fulfillment settlement over to the order payment port.
- [x] Close the currently identified checkout payment-settlement post-delegation
  mapper and canonical owner payload-diagnostic sites at source level without
  changing public envelopes, identity policy, settlement, replay, or payment
  identity behavior.
- [x] Close shared checkout write-admission and tenant/actor/causation rejection
  payload diagnostics at source level without changing admission severity,
  validation order, returned `PortError`, or owner delegation.
- [x] Remove direct `orders` SQL and direct `OrderService` construction from the
  staged order stage, mounted pipeline, compensation, and fulfillment settlement
  source.
- [x] Add focused SQLite source tests for journal reads/replay/contention,
  completion result reads/conflict, and owner-port legacy adoption.
- [x] Add static boundary verifiers for direct commerce order SQL, staged
  completion, typed recovery lifecycle, compensation, and payment/fulfillment
  owner-stage cutovers.
- [ ] Execute the full static verifier set against a repository checkout.
- [ ] Execute order/commerce compile and targeted Rust tests.
- [ ] Execute clean/upgraded/down/reapply migrations on SQLite, PostgreSQL, and
  MySQL and retain constraint/rollback evidence.
- [ ] Execute PostgreSQL/MySQL concurrent completion/admission, compensation,
  payment settlement, kill-point, restart, and remote-adapter evidence.
- [ ] Remove old JSON expression indexes, generated columns, metadata identity
  writes, old creation/confirmation/compensation/pipeline source, and
  `adopt_legacy` after every production consumer is cut over.

## Order projection read source checklist

- [x] Publish one owner `OrderReadPort` for complete order detail/list plus
  return and order-change detail/list projections.
- [x] Preserve `OrderResponse`, `OrderReturnResponse`, and `OrderChangeResponse`;
  page/per-page handling, owner totals, descending ordering, complete return items,
  and current status/customer/order/type filters.
- [x] Use `PortContext.locale` and explicit tenant-default fallback only for the
  currently localized complete-order projection requests; retain resolved request
  locale as context for non-localized post-order reads.
- [x] Require `PortCallPolicy::read()` and parse tenant identity from
  `PortContext` for all six operations.
- [x] Map every current `OrderError` variant to stable `PortError` policy without
  owner-message control flow.
- [x] Export the canonical `in_process_order_read_port` factory and all typed
  request/page contracts.
- [x] Retain source evidence and focused guards without claiming runtime parity.
- [x] Publish and host-compose `CommerceOrderReadRuntime` while preserving an
  externally installed runtime.
- [x] Require the host-selected runtime in Commerce HTTP and GraphQL schema-data
  composition.
- [x] Cut admin REST order list/detail over to the owner port while preserving
  locale, channel, deadline, filters, pagination total, detail aggregation, and
  public envelopes.
- [x] Cut GraphQL order list/detail over to the host-selected runtime through the
  mounted resolver scope while retaining an embedded-schema in-process fallback.
- [x] Propagate authenticated actor, request channel, and effective locale into
  GraphQL order read context; unauthenticated or embedded reads retain explicit
  service/no-channel and truthful unknown-locale fallback.
- [x] Cut storefront HTTP order detail and shared ownership reads over to the
  host-selected runtime while preserving customer resolution, locale fallback,
  public envelopes, and the concrete operations that follow ownership validation.
- [x] Cut storefront return and order-change list reads over to the host-selected
  runtime while preserving filters, ordering, complete DTOs, totals, and envelopes.
- [x] Cut GraphQL return/order-change detail and list reads over to the scoped
  host-selected runtime while preserving typed not-found error shapes and without
  changing mutations.
- [x] Cut mounted admin return/order-change detail and list reads over to the
  host-selected runtime while preserving `ORDERS_READ`, filters, clamped
  pagination, totals, public envelopes, actor/channel/locale/deadline context, and
  all mutation/payment/fulfillment ownership.
- [ ] Execute compile and mounted parity for the four admin post-order GET routes,
  then remove their unmounted compatibility handlers.
- [ ] Execute deadline/failure, restart, and remote-adapter evidence before status
  promotion.

## Open results

1. **Prove idempotent checkout completion.** Execute the owner command and staged
   consumer together under duplicate request, conflicting request, process-exit,
   restart, unknown lifecycle, and database contention scenarios.
   **Depends on:** compiled order/commerce crates and migrated test databases.
   **Done when:** one operation returns one placed order, inventory adoption
   resumes safely, and every mismatch or unknown lifecycle is a typed failure.

2. **Prove checkout order compensation.** Execute pending/confirmed/cancelled,
   identity mismatch, concurrent cancellation, process-exit, and upgraded legacy
   identity scenarios through the mounted commerce consumer.
   **Depends on:** compiled order/commerce crates and retained checkout identity
   migrations.
   **Done when:** replay returns one cancelled owner order, paid/shipped/delivered
   states require manual reconciliation, and commerce never reads order storage
   or constructs `OrderService`.

3. **Prove checkout payment settlement.** Execute confirmed-to-paid, already-paid,
   shipped/delivered replay, mismatched collection, mismatched payment reference,
   concurrent settlement, process-exit, and restart scenarios.
   **Depends on:** compiled order/payment/commerce crates and retained checkout
   identity rows containing payment collection facts.
   **Done when:** one captured payment identity settles one order, identical replay
   is read-only, and every conflicting identity fails closed.

4. **Remove the compatibility bridge.** Cut every remaining completion,
   recovery, admin, and remote consumer over to typed identity, then delete old
   metadata identity writes, JSON indexes/generated columns, legacy executors,
   unmounted compensation/pipeline source, and `adopt_legacy`.
   **Depends on:** upgraded migration and restart evidence for the staged cutover.
   **Done when:** no production lookup or lifecycle validation depends on
   `metadata.checkout.*`.

5. **Complete the post-order domain layer.** Evolve returns into explicit
   refund, exchange, claim, and order-change resolutions with owner-controlled
   lifecycle transitions and idempotent integration boundaries; do not move
   payment or fulfillment state transitions into this module.
   **Depends on:** published payment and fulfillment orchestration contracts.
   **Done when:** each resolution has typed references, failure semantics,
   outbox behavior, and targeted lifecycle tests.

6. **Prove checkout transport parity beyond the embedded owner path.** Keep
   GraphQL, native server-function, and remote-adapter behavior aligned for
   completion, identity, result, status, compensation, settlement, and full
   recovery projections.
   **Depends on:** the commerce checkout runtime and a remote adapter test
   environment.
   **Done when:** the contract-test matrix has executable remote evidence and
   fallback behavior supports a justified status promotion.

7. **Prove mounted order projection read parity.** Mounted GraphQL, storefront,
   and admin REST complete-order and post-order reads use the host-selected
   `CommerceOrderReadRuntime` without concrete `OrderService` projection reads on
   active routes.
   **Depends on:** compiled order/commerce/server crates and mounted local plus
   remote adapter profiles.
   **Done when:** locale/filter/pagination/authorization/ownership behavior,
   deadlines, stable failure policy, restart, and remote adapter parity are
   retained as execution evidence.

8. **Remove unmounted admin post-order compatibility handlers.** The active admin
   GET routes are cut over, but the superseded functions in `admin/returns.rs` and
   `admin/changes.rs` remain compiled as rollback-compatible source.
   **Depends on:** successful compile plus mounted route parity evidence.
   **Done when:** those four unmounted GET functions are deleted, OpenAPI remains
   unchanged, and no admin mutation or orchestration route moves ownership.

9. **Keep order and commerce documentation synchronized.** Update local docs,
   manifests, registries, central status, and the umbrella commerce plan whenever
   order lifecycle, checkout snapshots, identity ownership, or projection read
   ownership changes.
   **Done when:** no stale cross-module responsibility or evidence claim remains.

## Verification

- `npm run verify:ecommerce:fba`
- `node scripts/verify/verify-order-read-port.mjs`
- `node scripts/verify/verify-order-compensation-local-context.mjs`
- `node scripts/verify/verify-order-checkout-compensation-error-context.mjs`
- `node scripts/verify/verify-order-payment-settlement-local-context.mjs`
- `node scripts/verify/verify-order-payment-settlement-error-context.mjs`
- `node scripts/verify/verify-order-checkout-owner-context.mjs`
- `node scripts/verify/verify-commerce-graphql-order-read-shim.mjs`
- `node scripts/verify/verify-commerce-storefront-order-read-cutover.mjs`
- `node scripts/verify/verify-commerce-storefront-post-order-read-cutover.mjs`
- `node scripts/verify/verify-commerce-admin-post-order-read-cutover.mjs`
- `node scripts/verify/verify-commerce-admin-order-route-error-context.mjs`
- `node scripts/verify/verify-commerce-order-identity-boundary.mjs`
- `node --test scripts/verify/verify-commerce-order-identity-boundary.test.mjs`
- `node scripts/verify/verify-commerce-checkout-completion-cutover.mjs`
- `node --test scripts/verify/verify-commerce-checkout-completion-cutover.test.mjs`
- `node scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs`
- `node scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`
- `node scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs`
- `npm run verify:order:admin-boundary`
- `npm run verify:order:storefront-boundary`
- `npm run verify:commerce:storefront-transport-handoff`
- `cargo xtask module validate order`
- `cargo xtask module test order`
- `cargo check -p rustok-order --all-features`
- `cargo check -p rustok-commerce --lib`
- `cargo check -p rustok-server --features mod-commerce`
- `cargo test -p rustok-order --test order_checkout_identity`
- `cargo test -p rustok-order --test checkout_order_identity_port`
- `cargo test -p rustok-order --test checkout_completion_port`
- Targeted order read-port six-operation, locale fallback, context, error-policy,
  host-composition, admin, GraphQL, and storefront transport tests.
- Targeted staged checkout completion/adoption/replay, unknown lifecycle,
  compensation, and payment settlement tests.
- Clean/upgraded/down/reapply identity migrations on SQLite/PostgreSQL/MySQL.
- Concurrent completion/compensation/settlement, process-exit, restart, tenant
  mismatch, legacy adoption, remote profile, lifecycle, snapshot, and rollback
  tests.

No verification command was executed in this source wave.

## Change rules

1. Keep order writes and snapshots within this module; use public contracts for
   payment, fulfillment, and commerce orchestration.
2. Update local and umbrella commerce documentation in the same change as a
   cross-module order contract.
3. Update this status block and `docs/modules/registry.md` only with proven UI
   or FBA boundary changes.
4. Do not invent legacy checkout cart, hash, payment, shipping, actor, or
   provider facts during migration or adoption.
5. Keep compatibility lookup owner-local and delete it immediately after every
   production consumer uses typed identity.
