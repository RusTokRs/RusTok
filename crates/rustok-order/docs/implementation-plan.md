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
bind, and explicit legacy adoption over that owner state. Staged checkout order
creation and compensation now consume this port and no longer query `orders` or
inspect checkout identity metadata themselves.

Legacy order metadata remains a temporary compatibility input only inside the
order-owned `adopt_legacy` adapter. Legacy rows retain `NULL` for unknown cart or
missing hash facts rather than fabricating attribution. The metadata bridge and
old JSON indexes must be removed after completion/result consumers use typed
identity exclusively.

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
  `scripts/verify/verify-commerce-storefront-transport-handoff.mjs`, and
  `scripts/verify/verify-commerce-order-identity-boundary.mjs` lock the current
  UI, transport, and identity ownership split.
- No status promotion is allowed from source inspection. Clean/upgraded
  migrations, contention, restart, mounted consumers, and remote-profile
  evidence remain missing.

## Checkout identity workstream

- [x] Create owner-owned `order_checkout_identities` persistence without a
  foreign key to commerce-owned checkout tables.
- [x] Enforce one identity per checkout operation, order, and known source cart.
- [x] Enforce tenant/order consistency and immutable identity rows on
  PostgreSQL, SQLite, and MySQL source paths.
- [x] Backfill valid operation and hash facts from legacy metadata without
  inventing an unknown source cart or missing hashes.
- [x] Publish typed reads by operation/cart plus idempotent bind and explicit
  legacy adoption through `CheckoutOrderIdentityPort`.
- [x] Keep legacy metadata lookup inside the order-owned compatibility adapter;
  consumers receive only typed snapshots and safe `PortError` values.
- [x] Cut staged checkout order creation over to typed read/adopt/bind.
- [x] Cut checkout compensation identity resolution over to typed read/adopt.
- [x] Remove direct `orders` SQL and order metadata identity reads from commerce
  creation and compensation source.
- [x] Add focused SQLite source tests for journal reads/replay/contention and
  owner-port legacy adoption/cart conflict classification.
- [x] Add a static boundary verifier that rejects direct commerce order SQL,
  local legacy lookup helpers, and `order.metadata` identity reads.
- [ ] Execute the new journal/port tests and static verifier.
- [ ] Execute clean/upgraded/down/reapply migrations on SQLite, PostgreSQL, and
  MySQL and retain constraint/rollback evidence.
- [ ] Execute PostgreSQL/MySQL concurrent admission, legacy adoption, restart,
  and remote-adapter evidence.
- [ ] Move `CheckoutCompletionPort` create/place/result reads onto the typed
  identity so cart/operation recovery is complete.
- [ ] Remove old JSON expression indexes, generated columns, metadata identity
  writes, and `adopt_legacy` after every production consumer is cut over.

## Open results

1. **Complete idempotent checkout completion.** Replace the current separate
   create/confirm calls with one owner command bound to typed checkout identity,
   and implement result reads by operation/cart.
   **Depends on:** `CheckoutOrderIdentityPort` and staged checkout consumer
   cutover.
   **Done when:** concurrent and crash replay returns one placed order, result
   reads no longer return unavailable, and request-hash conflicts are typed.

2. **Remove the compatibility bridge.** Cut every remaining order completion,
   recovery, admin, and remote consumer over to typed identity, then delete old
   metadata identity writes, JSON indexes/generated columns, and
   `adopt_legacy`.
   **Depends on:** completed checkout port and upgraded migration evidence.
   **Done when:** no production lookup depends on `metadata.checkout.*`.

3. **Complete the post-order domain layer.** Evolve returns into explicit
   refund, exchange, claim, and order-change resolutions with owner-controlled
   lifecycle transitions and idempotent integration boundaries; do not move
   payment or fulfillment state transitions into this module.
   **Depends on:** published payment and fulfillment orchestration contracts.
   **Done when:** each resolution has typed references, failure semantics,
   outbox behavior, and targeted lifecycle tests.

4. **Prove checkout transport parity beyond the embedded owner path.** Keep
   GraphQL, native server-function, and remote-adapter behavior aligned for
   completion, identity, result, and status reads.
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
- `npm run verify:order:admin-boundary`
- `npm run verify:order:storefront-boundary`
- `npm run verify:commerce:storefront-transport-handoff`
- `cargo xtask module validate order`
- `cargo xtask module test order`
- `cargo test -p rustok-order --test order_checkout_identity`
- `cargo test -p rustok-order --test checkout_order_identity_port`
- Clean/upgraded/down/reapply identity migrations on SQLite/PostgreSQL/MySQL.
- Targeted identity replay, contention, tenant/order mismatch, legacy adoption,
  remote profile, lifecycle, snapshot, and rollback tests.

## Change rules

1. Keep order writes and snapshots within this module; use public contracts for
   payment, fulfillment, and commerce orchestration.
2. Update local and umbrella commerce documentation in the same change as a
   cross-module order contract.
3. Update this status block and `docs/modules/registry.md` only with proven UI
   or FBA boundary changes.
4. Do not invent legacy checkout cart, hash, actor, or provider facts during
   migration or adoption.
5. Keep compatibility lookup owner-local and delete it immediately after every
   production consumer uses typed identity.
