# M6 replay dead-letter admission

Status: `source_complete_operator_requeue_pending`

This slice makes terminal replay failure fail closed at schema-scope admission. A durable
`failed` rebuild job is treated as a dead letter instead of being ignored while a later
invocation creates a fresh job identity.

## Contract

`PostgresIndexReplayJobStore::acquire` still serializes the exact tenant/schema scope
before inspecting jobs. Selection now includes `failed` rebuild jobs and applies this
precedence:

1. `succeeded` proves the scope is already complete;
2. an existing `running` or `pending` job remains authoritative and may be busy or
   claimable;
3. when neither successful nor active work exists, the newest `failed` job blocks
   admission.

The blocked call returns `IndexReplayJobError::DeadLettered` with only:

- the existing job UUID;
- the durable attempt count;
- the optional bounded `last_error_code`.

`last_error_details`, source/database/transport causes, tenant data, request data, and
stack information are never returned from the admission error.

## Identity and retry boundary

A blocked scope does not insert another `index_jobs` row and does not reset the
checkpoint. The existing failed job remains the durable diagnostic and operator
decision point.

This contract complements the separate retry-transition store:

- retryable work may move the same job from `running` to delayed `pending`;
- permanent or exhausted work may move it to `failed`;
- once `failed` is authoritative and no active/succeeded job exists, ordinary replay
  acquisition cannot bypass it with a new job UUID.

Legacy scopes that already contain an active job plus an older failed job continue the
active job because active work has higher precedence. A retained `succeeded` job always
wins over older active or failed rows.

## Explicitly open

- an authorized operator inspect/requeue command;
- an audit contract for who requeued a dead letter and why;
- retry-budget reset or epoch semantics for manual requeue;
- host scheduling of eligible `pending` jobs;
- runner integration with the retry-transition store;
- retained PostgreSQL evidence for failure, retry exhaustion, restart, concurrent
  acquisition, and operator requeue.

The combined implementation-plan item for bounded retry/backoff, dead-letter state,
and global scheduling ownership remains open.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, workflow execution, and live
PostgreSQL validation are maintainer-run. The implementation agent did not execute
them.
