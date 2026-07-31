# Reconciliation mutation failure diagnostics PostgreSQL harness

Status: executable target retained, not run.

## Purpose

This harness exercises the canonical `PostgresIndexReconciliationRunner` mutation-failure branch against real Index migrations and PostgreSQL storage.

The existing source-failure harness proves bounded diagnostics for owner-source errors. This target covers the complementary failures returned by `PostgresMutationStore` through `IndexReconciliationRunError::MutationFailed`.

## Permanent validation failure

The source returns one page whose mutation remains inside the requested tenant/schema scope but omits the schema's required `id` field.

`IndexSourcePage` accepts that source-scope contract. Full schema validation occurs inside `PostgresMutationStore`, which must return the permanent replay failure code:

- `mutation_rejected`

The reconciliation call must return `IndexReconciliationRunError::MutationFailed` at position zero with `IndexReplayFailureKind::Permanent`.

The durable attempt must terminalize as `failed` with:

- attempt count 1;
- cursor `completed_passes = 0`;
- cursor `pages_processed = 0`;
- cleared lease owner and expiry;
- non-null completion timestamp;
- no entity or inbox row.

## Retryable storage failure

The valid source blocks behind an async barrier only after reconciliation acquisition has committed attempt 1.

A separate schema-scoped connection verifies the active job and then temporarily renames only `index_entities` inside the isolated test schema. The source is released after the rename.

Mutation persistence inserts its inbox candidate inside one transaction, then fails when it reads the unavailable entity table. The mutation transaction must roll back and classify the storage error as:

- `mutation_storage_retryable`;
- `IndexReplayFailureKind::Retryable`.

After the runner returns, the harness restores the table name before reading final evidence.

The failed attempt must retain zero cursor progress and no entity or inbox rows. This proves that the transient storage fault does not leave a partial inbox delivery.

## Bounded diagnostic contract

Both cases persist `last_error_code = index.reconciliation_page_failed` and exact three-field JSON:

```json
{
  "contract": "index_reconciliation_run_failure_v1",
  "dependency_code": "mutation_rejected",
  "retryable": false
}
```

or:

```json
{
  "contract": "index_reconciliation_run_failure_v1",
  "dependency_code": "mutation_storage_retryable",
  "retryable": true
}
```

The durable payload contains no database error text, SQL, table name, tenant, actor, worker, job, request, source record, transport detail, stack, or arbitrary mutation payload.

Later exact-tenant cancellation must observe `AlreadyTerminal(Failed)` for both jobs.

## PostgreSQL isolation

Each case reads `RUSTOK_INDEX_TEST_DATABASE_URL`, with PostgreSQL `DATABASE_URL` as a fallback. Without a PostgreSQL URL it reports a skip and succeeds.

Every invocation:

1. creates one unique PostgreSQL schema;
2. creates the tenant owner fixture;
3. applies every real `IndexModule` migration;
4. persists one active source-owned schema;
5. constructs the canonical source/schema registries and reconciliation runner;
6. reads exact durable job/entity/inbox evidence;
7. drops the isolated schema.

The retryable case restores `index_entities` before evidence reads and cleanup.

## Scope boundaries

This slice adds only test, documentation, and verifier files. It does not change production code, migrations, SQL/state-machine behavior, source/cursor contracts, mutation identity, diagnostics, schema fingerprints, or public API.

It does not add:

- automatic retry, backoff, jitter, scheduling, or attempt exhaustion;
- failed-scope dead-letter admission or operator requeue;
- source-call timeout or pending-future preemption;
- cancellation, heartbeat, lease-loss, or takeover races;
- process, host, container, or PostgreSQL restart evidence;
- digest comparison, orphan cleanup, targeted/full/shadow repair;
- locale or partition checkpoint dimensions;
- complete drift repair.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Suggested maintainer validation

```bash
RUSTOK_INDEX_TEST_DATABASE_URL=postgresql://... \
  cargo test -p rustok-index \
  --test source_reconciliation_mutation_failure_diagnostics_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-mutation-failure-diagnostics-harness.mjs
```

These commands were not run by the implementation agent.
