# Pages / Page Builder Outbox Relay Restart Packet

Date: 2026-08-04
Status: source-ready / PostgreSQL-execution-pending / FFA-FBA-not-promoted
Scope: durable Pages `NodePublished` retry → relay process restart → cache handler receipt → outbox acknowledgement
Canonical basis:

- `docs/modules/pages-page-builder-parity-continuation-plan.md`;
- `docs/modules/pages-page-builder-postgres-outbox-cache-packet-2026-08-04.md`.

## Cursor closed by this source slice

The previous PostgreSQL packet made the operation-receipt and durable-outbox path
executable but left relay acknowledgement and process-restart evidence open.

This slice adds an environment-gated PostgreSQL harness for that exact boundary. It
does not claim that PostgreSQL, Cargo or a verifier ran.

## Outbox relay restart packet: ready, unvalidated

`crates/rustok-pages/tests/outbox_relay_restart_postgres.rs` creates an isolated
PostgreSQL schema and applies the real `OutboxModule` migration. It writes one Pages
`NodePublished` root envelope through `TransactionalEventBus` and commits the durable
`sys_events` row before either relay worker starts.

The first relay worker uses the identity `pages-relay-before-restart`. Its target
returns one deliberate transient transport error before the Pages cache handler can
run. The harness retains the expected durable retry state:

- the outbox row remains `pending`;
- `retry_count` advances from zero to one;
- `last_error` and `next_attempt_at` are present;
- the claim identity and timestamp are cleared;
- `dispatched_at` remains absent;
- no event is recorded as delivered;
- route, page and artifact generations remain unchanged.

The second relay instance uses the distinct identity `pages-relay-after-restart`. It
reclaims the same pending row and delivers the same root envelope to the real
`PageCacheInvalidationEventHandler`.

The successful delivery retains:

- the exact durable event UUID;
- the root correlation UUID equal to that event UUID;
- one Pages invalidation request and one validated receipt;
- route, page and artifact generation rotation exactly once;
- one target delivery despite two relay attempts.

The outbox acknowledgement is persisted only after target delivery succeeds. The
final row is `dispatched`, has `dispatched_at`, has no claim, error or next-attempt
state, and retains the single prior retry count for diagnostics.

## Production boundaries

This slice does not change production Pages, Outbox or cache behavior. In particular:

- `OutboxRelay` still claims bounded pending batches under worker identity;
- target publication still precedes `mark_dispatched`;
- failed target publication still records retry state and clears the claim;
- successful target publication still clears error/retry scheduling and records
  `dispatched_at`;
- the Pages cache handler still validates event/correlation-bound receipts before
  returning success;
- no cache service is called by the test target before relay delivery succeeds;
- no migration, entity, public transport or HTTP route changes are introduced.

## Evidence

Source evidence is recorded in:

- `crates/rustok-pages/contracts/evidence/pages-outbox-relay-restart-postgres-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs`;
- `crates/rustok-pages/tests/outbox_relay_restart_postgres.rs`.

PostgreSQL execution remains pending. The evidence execution list is empty and all
validation flags remain false.

## Remaining work

The artifact HTTP packet remains open. A later bounded slice should retain actual
Axum artifact responses across generation miss, owner artifact read, cache refill,
cache hit and conditional `304 Not Modified`. The native storefront server-function
path should retain the equivalent authorization, generation, source and refill
ordering.

Relay crash-after-target-success but before durable acknowledgement also remains a
separate duplicate-delivery/idempotency packet; this slice covers transient failure
before target success and restart recovery from the durable pending row.

FFA/FBA promotion remains blocked on executed PostgreSQL, HTTP, browser, workflow and
rollout evidence.

## Suggested maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-outbox-relay-restart-postgres.mjs
RUSTOK_PAGES_TEST_DATABASE_URL=postgres://... \
  cargo test -p rustok-pages --test outbox_relay_restart_postgres -- --nocapture
cargo check -p rustok-pages --all-targets
```
