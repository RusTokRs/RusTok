# Implementation plan for `rustok-order`

Last reviewed: 2026-07-22

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

Order now also owns `order_checkout_identities`, an immutable typed projection
that binds a checkout operation to one order and, for live writes, one source
cart plus immutable snapshot/request hashes. Legacy rows are backfilled only
from valid facts already present in order metadata; an unknown legacy cart is
kept as `NULL` rather than fabricated. The old JSON identity path remains a
temporary compatibility bridge until commerce is cut over to owner reads.

## FFA/FBA boundary

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `order.checkout_completion.v1` in
  `crates/rustok-order/contracts/order-fba-registry.json`.
- Static contract evidence:
  `crates/rustok-order/contracts/evidence/order-contract-test-static-matrix.json`.
- `scripts/verify/verify-order-admin-boundary.mjs`,
  `scripts/verify/verify-order-storefront-boundary.mjs`, and
  `scripts/verify/verify-commerce-storefront-transport-handoff.mjs` lock the
  module/commerce UI and transport ownership split.
- Storefront owner transport uses `execute_selected_transport` with native
  `#[server]` selected first and GraphQL retained as the parallel fallback.
- No status promotion is allowed from the new identity source alone; clean /
  upgraded migrations, contention, replay, and mounted consumer evidence are
  still missing.

## Checkout identity workstream

- [x] Create owner-owned `order_checkout_identities` persistence without a
  foreign key to commerce-owned checkout tables.
- [x] Enforce one identity per checkout operation, order, and known source cart.
- [x] Enforce tenant/order consistency and immutable identity rows on
  PostgreSQL, SQLite, and MySQL source paths.
- [x] Backfill valid operation and hash facts from legacy metadata without
  inventing an unknown source cart or missing hashes.
- [x] Publish owner-local typed reads by operation, order, and cart.
- [x] Make owner-local identity admission replay-safe and classify conflicting
  operation/order/cart bindings as typed conflicts.
- [x] Add focused SQLite source tests for typed reads, replay, concurrency, and
  conflict classification.
- [ ] Execute clean/upgraded/down/reapply migrations on SQLite, PostgreSQL, and
  MySQL and retain constraint/rollback evidence.
- [ ] Execute PostgreSQL/MySQL concurrent admission and restart evidence.
- [ ] Cut commerce order creation and recovery over to this owner identity.
- [ ] Remove the old JSON expression indexes, generated column, metadata
  identity writes, and direct metadata lookup after all consumers are cut over.

## Open results

1. **Complete owner checkout identity cutover.** Publish the typed read/command
   contract over the owner journal, use it from staged checkout and
   compensation, then remove direct `orders` SQL and JSON identity lookup from
   commerce.
   **Depends on:** the typed identity migration and journal source.
   **Done when:** create/place/read replay uses owner ports, legacy adoption is
   explicit, and no production consumer queries checkout identity from metadata.

2. **Complete the post-order domain layer.** Evolve returns into explicit
   refund, exchange, claim, and order-change resolutions with owner-controlled
   lifecycle transitions and idempotent integration boundaries; do not move
   payment or fulfillment state transitions into this module.
   **Depends on:** published payment and fulfillment orchestration contracts.
   **Done when:** each resolution has typed references, failure semantics,
   outbox behavior, and targeted lifecycle tests.

3. **Prove checkout transport parity beyond the embedded owner path.** Keep
   GraphQL, native server-function, and remote-adapter fallback behavior aligned
   for completion, result, and status reads.
   **Depends on:** the commerce checkout runtime and a remote adapter test
   environment.
   **Done when:** the contract-test matrix has executable remote evidence and
   fallback behavior supports a justified status promotion.

4. **Keep order and commerce documentation synchronized.** Update local
   README/admin docs, manifest metadata, central registry, and the umbrella
   commerce plan whenever order lifecycle, checkout snapshots, or post-order
   ownership changes.
   **Depends on:** the change-owning contract.
   **Done when:** no owner-specific server GraphQL artifact, duplicate DTO, or
   stale cross-module responsibility remains.

## Verification

- `npm run verify:ecommerce:fba`
- `npm run verify:order:admin-boundary`
- `npm run verify:order:storefront-boundary`
- `npm run verify:commerce:storefront-transport-handoff`
- `cargo xtask module validate order`
- `cargo xtask module test order`
- `cargo test -p rustok-order --test order_checkout_identity`
- Clean/upgraded/down/reapply identity migrations on SQLite/PostgreSQL/MySQL.
- Targeted identity replay, contention, tenant/order mismatch, lifecycle,
  snapshot, outbox, and post-order tests.

## Change rules

1. Keep order writes and snapshots within this module; use public contracts for
   payment, fulfillment, and commerce orchestration.
2. Update local and umbrella commerce documentation in the same change as a
   cross-module order contract.
3. Update this status block and `docs/modules/registry.md` only with proven UI
   or FBA boundary changes.
4. Do not invent legacy checkout cart, hash, actor, or provider facts during
   migration.
5. Remove the metadata compatibility bridge immediately after all production
   consumers use the typed owner identity.
