# M6 replay dead-letter admission

Status: `source_complete_operator_requeue_pending`

## Purpose

This slice makes terminal replay failure fail closed at schema-scope admission. A durable
`failed` rebuild job becomes the ordinary replay admission barrier instead of being ignored while
a later invocation creates a fresh job identity.

`PostgresIndexReplayJobStore::acquire` retains its existing tenant/schema advisory lock and schema
registration checks. It changes only the set and precedence of matching replay jobs.

## Admission precedence

The job query includes `pending`, `running`, `succeeded`, and `failed` rows and orders them by
state authority before creation time:

1. a retained `succeeded` row proves the scope is already complete;
2. an existing `running` row remains authoritative active work;
3. an existing `pending` row remains authoritative delayed or immediately claimable work;
4. only when no successful or active work exists does the newest `failed` row block admission.

Expired `running` work and eligible `pending` work preserve their existing reclaim path. A delayed
retry produced by `PostgresIndexReplayRetryStore` remains `Busy` until
`available_at <= CURRENT_TIMESTAMP`; an older failed row cannot bypass that active retry state.

When the failed row is authoritative, acquisition returns
`IndexReplayJobError::DeadLettered` with only:

- the existing job UUID;
- the durable attempt count;
- the optional bounded and validated `last_error_code`.

The SELECT does not load `last_error_details`. Source/database/transport causes, tenant or request
values, mutation data, SQL, and stack text are not available through the admission error.

## Identity and retry boundary

A blocked scope inserts no new `index_jobs` row, changes no checkpoint, resets no attempt budget,
and does not mutate the failed job. The existing failed row remains the durable operator decision
point.

This contract complements the merged retry transition store:

- retryable work below the limit may move the same job from `running` to delayed `pending`;
- permanent or exhausted work may move the same job from `running` to `failed`;
- once `failed` is authoritative, ordinary acquisition cannot bypass it with a new job UUID.

The current replay runner can also terminalize a page failure through its existing failure path.
Dead-letter admission applies to either producer of a valid failed replay job; it does not claim
that runner failure classification is already wired into the retry store.

## Ownership boundaries

This slice does not add or claim:

- operator dead-letter listing or detail inspection;
- authorized requeue or a failed-to-pending transition;
- actor, reason, approval, or audit evidence for requeue;
- retry-budget reset or retry-epoch semantics;
- host scheduling of eligible pending jobs;
- replay-runner integration with `PostgresIndexReplayRetryStore`;
- graceful scheduler shutdown or fleet coordination;
- retained PostgreSQL failure, exhaustion, restart, concurrent admission, or requeue evidence;
- migration, table-shape, source, cursor, mutation, checkpoint, or event-identity changes.

A later requeue contract must authorize the exact tenant and schema scope, identify the blocked
job, define attempt/epoch semantics, retain actor and reason audit data, and preserve concurrent
admission fencing. This source slice deliberately provides none of those mutation capabilities.

The canonical implementation-plan item `Add bounded retry/backoff, dead-letter state, and global
scheduling ownership` remains open.

## Source evidence

Focused source coverage retains:

- one terminal failed job blocks a second ordinary acquisition;
- the returned error exposes only job UUID, attempt count, and bounded error code;
- private `last_error_details` are not returned;
- no second rebuild job row is created for the blocked scope;
- existing success, busy, reclaim, checkpoint, schema, source, and stored-request invariants remain
  in the same fixture suite.

Formatting, Cargo checks/tests, JavaScript verifiers, live PostgreSQL scenarios, workflows, and CI
are maintainer-run and were not executed by the implementation agent.
