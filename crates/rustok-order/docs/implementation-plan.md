# Implementation plan for `rustok-order`

Last reviewed: 2026-07-23

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
the same typed request/result contract.

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

Captured checkout payment settlement invokes
`CheckoutOrderPaymentSettlementPort`. Order owner validates checkout, cart, order,
and payment-collection identity; transitions a confirmed order to paid; and adopts
paid, shipped, or delivered replay only when the payment reference and method
match. The mounted commerce fulfillment stage no longer constructs `OrderService`.

Legacy order metadata remains a temporary compatibility input only inside
order-owned adapters. Legacy rows retain `NULL` for unknown cart, payment,
shipping, or hash facts rather than fabricating attribution. The metadata bridge
and old JSON indexes must be removed after all completion/result consumers use
typed identity exclusively.

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
- Static contract evidence:
  `crates/rustok-order/contracts/evidence/order-contract-test-static-matrix.json`.
- `scripts/verify/verify-order-admin-boundary.mjs`,
  `scripts/verify/verify-order-storefront-boundary.mjs`,
  `scripts/verify/verify-commerce-storefront-transport-handoff.mjs`,
  `scripts/verify/verify-commerce-order-identity-boundary.mjs`,
  `scripts/verify/verify-commerce-checkout-completion-cutover.mjs`,
  `scripts/verify/verify-commerce-checkout-compensation-owner-boundary.mjs`,
  `scripts/verify/verify-commerce-checkout-owner-stage-boundary.mjs`, and
  `scripts/verify/verify-ecommerce-typed-lifecycle-statuses.mjs` lock the current
  UI, transport, identity, staged-consumer, lifecycle, compensation, and payment
  settlement split.
- No status promotion is allowed from source inspection. Clean/upgraded
  migrations, compile/tests, contention, restart, mounted consumers, and
  remote-profile evidence remain missing.

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
- [x] Publish `CheckoutOrderPaymentSettlementPort` with typed checkout/payment
  identity, owner-local settlement, replay adoption, and payment-reference
  conflict classification.
- [x] Cut mounted checkout fulfillment settlement over to the order payment port.
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

7. **Keep order and commerce documentation synchronized.** Update local docs,
   manifests, registries, central status, and the umbrella commerce plan whenever
   order lifecycle, checkout snapshots, or identity ownership changes.
   **Done when:** no stale cross-module responsibility or evidence claim remains.

## Verification

- `npm run verify:ecommerce:fba`
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
- `cargo test -p rustok-order --test order_checkout_identity`
- `cargo test -p rustok-order --test checkout_order_identity_port`
- `cargo test -p rustok-order --test checkout_completion_port`
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
