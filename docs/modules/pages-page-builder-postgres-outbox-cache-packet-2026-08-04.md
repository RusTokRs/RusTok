# Pages / Page Builder PostgreSQL Outbox and Cache Packet

Date: 2026-08-04
Status: source-ready / PostgreSQL-execution-pending / FFA-FBA-not-promoted
Scope: reviewed publish and immutable rollback operation receipt → durable `NodePublished` → Pages cache generation rotation
Canonical basis: `docs/modules/pages-page-builder-parity-continuation-plan.md`

## Cursor closed by this source slice

The canonical parity plan left open a database/outbox/cache packet tying real publish
and rollback operation receipts to durable `NodePublished` envelopes, handler
receipts, generation changes and storefront/artifact miss-refill observations.

This overlay makes that packet executable without claiming that it ran.

## PostgreSQL receipt/outbox/cache packet: ready, unvalidated

`crates/rustok-pages/tests/publish_rollback_outbox_cache_postgres.rs` creates one
isolated PostgreSQL schema and applies the real `OutboxModule` and `PagesModule`
migrations. It does not create replacement operation or outbox tables.

The harness uses `TransactionalEventBus` over `OutboxTransport`. Its publish fixture:

1. starts one PostgreSQL transaction;
2. advances the page to published version 2;
3. writes a root `NodePublished` envelope to `sys_events`;
4. inserts the durable `page_publish_operations` receipt;
5. commits the transaction.

Its rollback fixture repeats the same ownership order for published page version 3
and `page_rollback_operations`.

The harness reads each envelope back from `sys_events`, validates the registered event
schema and requires the root correlation id to equal the durable event id. The decoded
envelope, rather than a separately constructed replacement event, is passed to the
real `PageCacheInvalidationEventHandler`.

For both durable publish and rollback envelopes the cache cycle requires:

- exact event and correlation identities on the handler request and receipt;
- one route, page and artifact generation increment;
- a miss under the new storefront and artifact keys;
- refill followed by a hit under those new keys;
- the previous generation values to remain physically present but unreachable through
  current key construction.

A separate receipt-conflict transaction writes the outbox envelope first, then forces
the publish receipt insert to fail on the durable idempotency constraint. The
transaction is rolled back and the harness requires the already inserted outbox row
to be absent. This is the retained atomicity boundary for event-plus-receipt failure.

Production reviewed publish and immutable rollback remain source-locked to the same
page mutation → `NodePublished` → operation receipt → commit order. Neither owner
calls cache services inline.

## Evidence

- harness:
  `crates/rustok-pages/tests/publish_rollback_outbox_cache_postgres.rs`;
- machine contract:
  `crates/rustok-pages/contracts/evidence/pages-publish-rollback-outbox-cache-postgres-source.json`;
- focused verifier:
  `crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs`;
- shared cache guard:
  `crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs`.

## Evidence state

The packet is source-ready only. PostgreSQL execution remains pending. No durable
outbox row, cache handler receipt, storefront response or artifact HTTP response is
claimed as observed by this update.

All machine validation flags remain false. The source slice does not promote Pages or
Page Builder FFA/FBA status.

## Boundaries

This slice does not:

- change production publish, rollback, outbox, cache or reader behavior;
- add or alter a database migration, entity, DTO, GraphQL or HTTP contract;
- add inline invalidation to lifecycle owners;
- claim relay delivery, retry, acknowledgement or restart evidence;
- execute PostgreSQL, browser, storefront, artifact HTTP, workflow or CI scenarios.

## Next cursor

1. Run and retain the PostgreSQL harness with
   `RUSTOK_PAGES_TEST_DATABASE_URL`.
2. Convert the resulting durable ids, receipt versions and generation snapshots into
   an accepted execution evidence packet.
3. Retain real storefront and artifact HTTP requests proving miss, source read, refill
   and subsequent hit after the durable publish and rollback envelopes.
4. Retain relay acknowledgement/restart behavior before promoting cache lifecycle
   evidence beyond source-ready.
5. Complete the previously open metadata browser and workflow/rollout packets before
   promoting FFA/FBA status.

## Maintainer validation

Suggested commands, intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-cache-invalidation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-cache-correlation.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test publish_rollback_outbox_cache_postgres -- --nocapture
cargo check -p rustok-pages --all-targets
```
