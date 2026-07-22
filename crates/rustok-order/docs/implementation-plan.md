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
- Published provider ports: `CheckoutCompletionPort` and
  `CheckoutOrderIdentityPort`.
- Static contract evidence:
  `crates/rustok-order/contracts/evidence/order-contract-test-static-matrix.json`.
- `scripts/verify/verify-order-admin-boundary.mjs`,
  `scripts/verify/verify-order-storefront-boundary.mjs`,
  `scripts/verify/verify-commerce-storefront-transport-handoff.mjs`,
  `scripts/verify/verify-commerce-order-identity-boundary.mjs`, and
  `scripts/verify/verify-commerce-checkout-completion-cutover.mjs` lock the
  current UI, transport, identity, and staged-consumer ownership split.
- No status promotion is allowed from source inspection. Clean/upgraded
  migrations, compile/tests, contention, restart, mounted consumers, and
  remote-profile evidence remain missing.

## Checkout identity and completion workstream

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
- [x] Cut checkout compensation identity resolution over to typed read/adopt.
- [x] Remove direct `orders` SQL and direct `OrderService` construction from the
  staged order stage and pipeline source.
- [x] Add focused SQLite source tests for journal reads/replay/contention,
  completion result reads/conflict, and owner-port legacy adoption.
- [x] Add static boundary verifiers for direct commerce order SQL and staged
  completion cutover; the new verifier unit fixtures pass 3/3.
- [ ] Execute the full static verifier set against a repository checkout.
- [ ] Execute order/commerce compile and targeted Rust tests.
- [ ] Execute clean/upgraded/down/reapply migrations on SQLite, PostgreSQL, and
  MySQL and retain constraint/rollback evidence.
- [ ] Execute PostgreSQL/MySQL concurrent completion/admission, kill-point,
  restart, and remote-adapter evidence.
- [ ] Remove old JSON expression indexes, generated columns, metadata identity
  writes, old creation/confirmation executor source, and `adopt_legacy` after
  every production consumer is cut over.

## Open results

1. **Prove idempotent checkout completion.** Execute the owner command and staged
   consumer together under duplicate request, conflicting request, process-exit,
   restart, and database contention scenarios.
   **Depends on:** compiled order/commerce crates and migrated test databases.
   **Done when:** one operation returns one placed order, inventory adoption
   resumes safely, and every mismatch is a typed conflict.

2. **Remove the compatibility bridge.** Cut every remaining completion,
   recovery, admin, and remote consumer over to typed identity, then delete old
   metadata identity writes, JSON indexes/generated columns, legacy executors,
   and `adopt_legacy`.
   **Depends on:** upgraded migration and restart evidence for the staged cutover.
   **Done when:** no production lookup or lifecycle validation depends on
   `metadata.checkout.*`.

3. **Complete the post-order domain layer.** Evolve returns into explicit
   refund, exchange, claim, and order-change resolutions with owner-controlled
   lifecycle transitions and idempotent integration boundaries; do not move
   payment or fulfillment state transitions into this module.
   **Depends on:** published payment and fulfillment orchestration contracts.
   **Done when:** each resolution has typed references, failure semantics,
   outbox behavior, and targeted lifecycle tests.

4. **Prove checkout transport parity beyond the embedded owner path.** Keep
   GraphQL, native server-function, and remote-adapter behavior aligned for
   completion, identity, result, status, and full recovery projections.
   **Depends on:** the commerce checkout runtime and a remote adapter test
   environment.
   **Done when:** the contract-test matrix has executable remote evidence and
   fallback behavior supports a justified status promotion.

5. **Keep order and commerce documentation synchronized.** Update local docs,
   manifests, registries, central status, and the umbrella commerce plan whenever
   order lifecycle, checkout snapshots, or identity ownership changes.
   **Done when:** no stale cross-module responsibility or evidence claim remains.

## Verification

- `npm run verify:ecommerce:fba`
- `node scripts/verify/verify-commerce-order-identity-boundary.mjs`
- `node --test scripts/verify/verify-commerce-order-identity-boundary.test.mjs`
- `node scripts/verify/verify-commerce-checkout-completion-cutover.mjs`
- `node --test scripts/verify/verify-commerce-checkout-completion-cutover.test.mjs`
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
- Targeted staged checkout completion/adoption/replay tests.
- Clean/upgraded/down/reapply identity migrations on SQLite/PostgreSQL/MySQL.
- Concurrent completion, process-exit, restart, tenant mismatch, legacy adoption,
  remote profile, lifecycle, snapshot, and rollback tests.

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
