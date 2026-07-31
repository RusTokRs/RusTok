# Reconciliation dead-letter admission

Status: production admission and PostgreSQL regression retained, not run.

## Boundary

A terminal failed reconciliation job now blocks later runs for the same tenant and schema scope.

The acquisition transaction continues to take the existing reconciliation scope advisory lock and verify the persisted schema before reading jobs. Scope resolution includes `failed` rows and uses deterministic precedence:

1. a retained `succeeded` row remains authoritative completion;
2. an existing `running` or `pending` row remains authoritative active work;
3. only when no successful or active row exists does the newest `failed` row block admission.

The runner returns `IndexReconciliationRunError::DeadLettered` instead of silently inserting a new reconciliation job.

## Bounded error contract

`DeadLettered` exposes only:

- the failed job UUID;
- its durable attempt count;
- its optional validated `last_error_code`.

The acquisition query does not load `last_error_details`. Database, source, mutation, transport, tenant, worker, request, stack, and arbitrary diagnostic payloads are not returned through the error.

Stored error codes must be non-empty, trimmed, free of control characters, and at most 128 bytes. An invalid stored code fails closed through `InvalidStoredJob` rather than being propagated.

The current page-failure transition stores `index.reconciliation_page_failed` as the bounded error code. The source or mutation dependency code remains inside the separately persisted bounded diagnostic object and is not part of dead-letter admission.

## PostgreSQL regression

The environment-gated target creates a unique schema, creates a tenant fixture, applies every real `IndexModule` migration, persists one active schema, and constructs the canonical source and schema registries.

A counted source fails its first scan with permanent owner code `owner_source_permanent_dead_letter`. The runner must create exactly one attempt-1 reconciliation job and terminalize it as failed with:

- zero completed passes and zero processed pages;
- released lease ownership;
- non-null completion timestamp;
- `last_error_code = index.reconciliation_page_failed`;
- the existing three-field reconciliation failure diagnostic;
- zero entity and inbox writes.

The test then replaces only `last_error_details` with a private marker. A newly constructed runner invokes the same scope and must return `DeadLettered` with the same job UUID, attempt count `1`, and only the generic page-failure code.

The second invocation must not call the source, create another job, change the failed row, or expose either the private marker or owner dependency code through Debug or Display output.

## Compatibility

This slice changes no migration, table shape, request or cursor wire contract, source ownership, mutation identity, schema fingerprint, failure transition, cancellation transition, lease fencing, or terminal success behavior.

It mirrors the fail-closed replay dead-letter admission boundary while remaining independently owned by reconciliation.

## Remaining work

This is admission only. It does not add:

- authorized dead-letter inspection;
- actor/reason audit records;
- manual requeue or retry-epoch reset;
- automatic retry, backoff, exhaustion, or scheduling;
- host polling or graceful task shutdown;
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
  --test source_reconciliation_dead_letter_admission_postgres_test \
  -- --nocapture

cargo check -p rustok-index --all-targets

node scripts/verify/verify-index-reconciliation-dead-letter-admission.mjs
```
