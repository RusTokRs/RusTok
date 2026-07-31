# M6 reconciliation failure diagnostics PostgreSQL harness

Status: executable target retained, not run.

## Purpose

`source_reconciliation_failure_diagnostics_postgres_test` retains PostgreSQL evidence for the existing reconciliation page-failure terminal transition. It verifies both source-failure classifications without changing production code:

- a permanent owner-source failure;
- a retryable owner-source failure.

Both cases use the canonical `PostgresIndexReconciliationRunner`, immutable source/schema registries, the real `IndexModule` migration sequence, and one persisted active schema.

## Durable transition

Each source fails on the first scan after the runner has acquired attempt 1. The call returns the typed `IndexReconciliationRunError::Source(IndexSourceError::SourceFailure { .. })` to its caller.

PostgreSQL must retain exactly one reconciliation job with:

- state `failed`;
- attempt count 1;
- cursor `completed_passes = 0`;
- cursor `pages_processed = 0`;
- cleared lease owner and lease expiry;
- non-null completion timestamp;
- `last_error_code = index.reconciliation_page_failed`.

No mutation was returned, so `index_entities` and `index_inbox` must both remain empty.

A later exact-tenant cancellation request must observe `AlreadyTerminal(Failed)`.

## Bounded diagnostic contract

`last_error_details` must equal exactly:

```json
{
  "contract": "index_reconciliation_run_failure_v1",
  "dependency_code": "<bounded source code>",
  "retryable": true
}
```

The permanent case stores `retryable: false`; the retryable case stores `retryable: true`.

The object must contain exactly those three fields. It does not retain:

- tenant or actor identity;
- worker or job identity;
- source name;
- schema or request payload;
- database, transport, stack, or arbitrary owner details.

The source failure itself already accepts only a validated machine-readable code. The harness therefore proves the persisted shape is bounded; it does not claim redaction of an unbounded source-detail field because no such field exists in the source contract.

## PostgreSQL isolation

The target reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Every case creates a unique schema, creates the tenant owner fixture, applies all real Index migrations, persists one active schema, uses one-connection pools with schema-local `search_path`, reads durable evidence, and drops the isolated schema.

## Scope boundaries

This harness does not add or prove:

- automatic retry, backoff, jitter, scheduling, or polling;
- retryable failure returning to `pending`;
- failed-scope dead-letter admission or operator requeue;
- attempt-budget exhaustion;
- cancellation or lease-loss races;
- process, host, container, or PostgreSQL restart;
- source-call timeout or source future preemption;
- digest comparison, orphan cleanup, targeted repair, or complete drift repair;
- locale or partition reconciliation checkpoint dimensions;
- server authorization, transport, task ownership, or graceful shutdown.

The existing reconciliation runner terminalizes retryable and permanent page failures identically as `failed`; only the bounded diagnostic `retryable` marker differs. Automatic retry ownership remains open.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested maintainer commands

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_failure_diagnostics_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-failure-diagnostics-harness.mjs
```

No Cargo, PostgreSQL, JavaScript verifier, Clippy, or CI result is recorded by this slice.
