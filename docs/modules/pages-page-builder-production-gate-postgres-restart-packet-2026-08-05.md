# Pages / Page Builder production gate PostgreSQL restart packet

Date: 2026-08-05
Status: source-ready / execution-pending

## Purpose

Retain one PostgreSQL server integration source that joins the historical owner-transaction and relay-restart packets to the production Pages generation gate introduced after those packets were written.

The historical PostgreSQL publish/rollback packet remains authoritative for receipt/outbox transaction ordering and receipt-conflict rollback. The historical relay-restart packet remains authoritative for pre-handler target failure and durable relay retry state. Neither historical harness mounts `TenantGenerationDeliveryGate` or `ServerPagesCachePort`.

This packet adds the missing production topology without replacing those owner-focused proofs.

## Retained topology

```text
PostgreSQL publish transaction
  → page version 2
  → durable NodePublished
  → durable publish receipt
  → commit
OutboxRelay
  → TenantGenerationDeliveryGate
  → ServerPagesCachePort
  → route/page/artifact generations 0/0/0 → 1/1/1
  → downstream acceptance
  → outbox acknowledgement
  → new storefront/artifact keys miss and refill

PostgreSQL rollback transaction
  → page version 3
  → durable NodePublished
  → durable rollback receipt
  → commit
first relay worker
  → production gate rotates 1/1/1 → 2/2/2
  → downstream rejects after rotation
  → outbox row remains pending with retry_count=1
second relay instance
  → same event UUID reaches production gate
  → process-bounded dedupe returns current receipt without another bump
  → downstream accepts
  → same durable row becomes dispatched
ordinary Pages module listener
  → same event UUID is a rotation no-op
  → new storefront/artifact keys refill while old values remain physically present
```

## Source retained

- `apps/server/tests/pages_production_gate_postgres_restart.rs`
- `crates/rustok-pages/contracts/evidence/pages-production-gate-postgres-restart-source.json`
- `crates/rustok-pages/scripts/verify/verify-pages-production-gate-postgres-restart.mjs`

The harness is gated by `RUSTOK_PAGES_TEST_DATABASE_URL`, with `DATABASE_URL` as fallback. Each run creates an isolated PostgreSQL schema, applies the real `OutboxModule` and `PagesModule` migrations, and drops the schema at completion.

## Important distinction

This packet deliberately models a **post-invalidation downstream failure**. The first failed rollback attempt has already rotated Pages generations before `OutboxRelay` persists retry state. A new relay instance retries the same durable envelope in the same process. The process-bounded dedupe prevents a second rotation while allowing downstream delivery and outbox acknowledgement to complete.

A process restart is still conservative rather than exact-once: process-bounded dedupe is not durable, so a replay after process restart may rotate once more. This remains safe because old keys stay unreachable from the current generation snapshot.

## Boundaries

This source slice does not:

- change production Pages, Page Builder, Outbox or cache code;
- change database schemas, event schemas, DTOs, routes, cache namespaces, key shape or TTL;
- replace the reviewed publish/rollback owner services with this fixture;
- claim that PostgreSQL, the verifier, Cargo, formatting, runtime profiles, HTTP routes, browsers, workflows or CI were executed;
- promote Pages FFA or Page Builder FBA.

## Maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-production-gate-postgres-restart.mjs
node crates/rustok-pages/scripts/verify/verify-pages-publish-rollback-outbox-cache-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs

cargo test -p rustok-server --features mod-pages \
  --test pages_production_gate_postgres_restart -- --nocapture
cargo check -p rustok-server --features mod-pages --all-targets
```

Execution evidence remains pending.
