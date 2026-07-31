# M6 reconciliation event-ID contract PostgreSQL harness

Status: executable target retained, not run.

## Purpose

This harness retains PostgreSQL evidence for the page-wide event identity preflight in
`PostgresIndexReconciliationRunner`.

The runner validates every mutation event UUID before it starts the mutation persistence
loop. A later invalid event UUID must therefore reject the complete page without allowing
an earlier valid mutation to create an entity or inbox row.

## Cases

The source page contains two schema-valid upsert mutations in the exact requested tenant
and schema scope. Both records contain the required `id` field and use distinct entity
UUIDs.

### Nil second event UUID

The first mutation has a non-nil event UUID. The second mutation has `Uuid::nil()`.

The run must return:

- `IndexReconciliationRunError::NilEventId`;
- position `1`;
- no mutation persistence.

### Duplicate second event UUID

The first and second mutations have distinct entity UUIDs but share the same non-nil
event UUID.

The run must return:

- `IndexReconciliationRunError::DuplicateEventId`;
- position `1`;
- the exact repeated event UUID;
- no mutation persistence.

## Durable failure evidence

Each case creates one attempt-1 reconciliation job and terminalizes it as `failed`.
The durable cursor remains at the initial safe boundary:

- `completed_passes = 0`;
- `pages_processed = 0`.

The job must retain:

- `last_error_code = index.reconciliation_page_failed`;
- cleared lease owner and expiry;
- non-null completion timestamp.

The diagnostic must equal this exact three-field JSON object:

```json
{
  "contract": "index_reconciliation_run_failure_v1",
  "dependency_code": "reconciliation_contract_invalid",
  "retryable": false
}
```

No event UUID, entity UUID, tenant UUID, worker ID, request, source payload, database
error, SQL, transport detail, or stack text is persisted in the diagnostic.

PostgreSQL must contain exactly:

- one reconciliation job;
- zero `index_entities` rows;
- zero `index_inbox` rows.

The zero-write requirement is material because the invalid identity is on the second
mutation. The first mutation is otherwise valid and would be persistable if the runner
applied mutations before completing page-wide event-ID validation.

A cancellation request from another tenant must return `NotFound`. A later exact-tenant
request must return `AlreadyTerminal(Failed)`.

## PostgreSQL isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a
fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Each case:

1. creates a unique PostgreSQL schema;
2. creates the tenant owner fixture;
3. applies every real `IndexModule` migration;
4. persists one active schema contract;
5. materializes canonical source and schema registries;
6. runs the canonical reconciliation runner;
7. reads durable job/entity/inbox evidence;
8. drops the isolated schema.

No sleep, polling delay, elapsed-time expiry, or concurrent race is used.

## Scope boundaries

This harness adds no production code, migration, reconciliation SQL/state-machine,
source/cursor contract, mutation identity, schema, diagnostic, or public API change.

It does not add or claim:

- automatic retry, backoff, scheduling, or attempt exhaustion;
- failed-scope dead-letter admission or authorized requeue;
- cancellation, heartbeat, lease-loss, takeover, or restart races;
- source-call timeout or pending-future preemption;
- cross-page event-ID uniqueness;
- source/index digest comparison;
- orphan cleanup;
- targeted, full, or shadow repair;
- locale or partition checkpoint dimensions;
- complete drift repair.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested validation

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_event_id_contract_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-event-id-contract-harness.mjs
```
