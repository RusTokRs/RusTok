# M6 reconciliation dead-letter admission

Status: `source_complete_operator_recovery_pending`.

## Purpose

A terminal failed reconciliation job now blocks later ordinary runs for the same exact tenant and schema scope. The runner no longer treats terminal failure as permission to silently insert a fresh reconciliation job identity.

The acquisition transaction keeps its existing boundaries:

1. take the tenant/schema reconciliation advisory lock;
2. verify that the persisted schema is present and active;
3. resolve retained jobs for the exact scope;
4. claim existing eligible work or create a new job only when no authoritative retained row exists.

## Admission precedence

Scope selection includes `pending`, `running`, `succeeded`, and `failed` rows and orders them by state authority before creation time:

1. retained `succeeded` work remains authoritative completion;
2. existing `running` work remains authoritative active work;
3. existing `pending` work remains authoritative delayed or claimable work;
4. only when no successful or active row exists does the newest `failed` row block admission.

Expired running work and eligible pending work keep their existing reclaim path. A failed row cannot bypass a newer or otherwise authoritative active job.

When failure is authoritative, acquisition returns `IndexReconciliationRunError::DeadLettered` with only:

- the retained job UUID;
- its durable attempt count;
- its optional bounded and validated `last_error_code`.

The blocked invocation does not call the source, insert another `index_jobs` row, reset attempt state, change the cursor, or mutate the failed row.

## Privacy boundary

The acquisition query selects `last_error_code` but deliberately does not select `last_error_details`.

Stored error codes must be non-empty, trimmed, free of control characters, and at most 128 bytes. Invalid stored codes fail closed through `InvalidStoredJob` rather than being returned.

Database causes, source or mutation dependency details, arbitrary diagnostic JSON, tenant/request values, worker and lease values, SQL, transport context, and stack text are not available through the dead-letter admission error.

The current page-failure transition stores `index.reconciliation_page_failed` as the public machine code. The bounded source or mutation dependency code remains inside the separately persisted reconciliation failure diagnostic and is not returned by ordinary admission.

## Retained PostgreSQL target

The environment-gated target creates a unique PostgreSQL schema, creates one tenant, applies the real `IndexModule` migrations, persists an active schema, and composes the canonical source and schema registries.

A counted source fails its first scan permanently. The target retains evidence that the runner creates exactly one attempt-1 reconciliation job and terminalizes it with released lease ownership, a completion timestamp, zero processed pages, no entity/inbox writes, the generic page-failure code, and the bounded reconciliation diagnostic.

The target then replaces only `last_error_details` with a private marker and invokes the same scope through a newly constructed runner. Admission must return the same failed job UUID and attempt count without calling the source again, creating another job, mutating the failed row, or exposing the private marker or dependency code through Debug or Display output.

This target is retained source evidence only; it was not executed by the implementation agent.

## Compatibility and ownership

This slice changes no migration, table shape, request or cursor wire contract, source ownership, mutation identity, schema fingerprint, failure transition, cancellation transition, lease fence, success behavior, or server authorization surface.

The guarded server reconciliation runtime merged separately and can surface this typed runner error only after its existing request-bound tenant/actor and `modules:manage` authorization. This slice does not add a transport or operator recovery command.

## Explicitly open

- bounded authorized dead-letter inspection;
- actor/reason audit records;
- manual requeue or retry-epoch reset;
- automatic retry, backoff, exhaustion, or scheduling;
- host polling, takeover discovery, or graceful shutdown;
- source/index digest comparison and orphan diagnosis;
- targeted, full, or shadow repair admission;
- locale or partition checkpoint dimensions;
- retained multi-instance PostgreSQL recovery evidence;
- complete drift repair.

The canonical M6 drift-diagnosis and targeted-repair roadmap item remains open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, PostgreSQL scenarios, workflows, and CI are maintainer-run and were not executed by the implementation agent.
