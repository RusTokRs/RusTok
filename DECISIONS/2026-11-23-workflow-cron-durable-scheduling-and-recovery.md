# Workflow cron: durable fire identity, cross-replica admission and fail-closed recovery

- Date: 2026-11-23
- Decision status: Proposed (maintainer review required)
- Implementation status: In progress
- Owners: rustok-workflow; host composition in apps/server
- Extends: 2026-11-12-durable-side-effect-recovery.md
- Supersedes: None
- Superseded by: None

## Context

The Workflow cron scheduler previously polled on a one-second wall-clock
window, spawned task-local executions and passed `trigger_event_id = None`
to `WorkflowEngine`. Separate replicas could fire the same workflow/slot,
and downtime silently lost missed slots. A crash left `running` step receipts,
possibly after a remote HTTP/notification effect but before the local receipt.
The event-triggered execution unique key does not cover cron without an
owner-issued stable trigger identity.

An expired lease or elapsed timestamp cannot prove that an in-flight external
effect stopped. Blind retry would violate the durable side-effect recovery
decision, including its explicit reconciliation requirement.

## Decision

1. `rustok-workflow` owns durable cron schedule cursors and firings. A
   persistent per-workflow schedule cursor records the exact schedule revision
   (trigger expression and workflow update version), last enqueued slot and a
   monotonically incrementing compare-and-set generation. Enqueue and cursor
   advancement commit in **one database transaction**; concurrent replicas
   either win that CAS or retry a future pass. A unique constraint on
   `(workflow_id, schedule_revision, scheduled_at)` prevents duplicate slots.
2. A bounded, UTC cron expansion advances from the cursor rather than comparing
   only the last second. New or changed schedules seed their cursor at the
   greater of workflow creation/update time and a configured bounded catch-up
   horizon; old pending firings are never reinterpreted as a new schedule
   revision. Each tick has a bounded workflow page and slot budget, so backlogs
   converge over subsequent ticks without unbounded memory or query work.
3. Each queued firing has a stable UUID trigger identity and a unique status
   transition `pending -> running` by conditional update; a current owner
   attempt token is stored with it. The Workflow engine receives the firing
   UUID as its `trigger_event_id`, using the existing unique
   `(workflow_id, trigger_event_id)` constraint. This prevents duplicate
   execution records, not independent remote effects.
4. Completing a firing requires the same attempt token. A stalled `running`
   firing, orphaned execution receipt or ambiguous failure becomes
   `reconciliation_required` and is never automatically requeued on time
   alone. Do not change an old `running` execution to `completed` without
   the owner observing its actual terminal result. Late fenced completion
   must not override an operator/recovery decision.
5. The first cutover only automatically dispatches firings still `pending`.
   It does not silently replay persisted `running` workflow step executions.
   An operator-facing recovery decision requires authorized human identity,
   durable evidence, and previous-worker termination attestation for ambiguous
   external effects. `requeue_confirmed_unstarted` is valid only when no
   execution journal was ever committed. `confirm_terminal` is valid for an
   existing terminal execution. An operator may also record a reconciled
   overall execution outcome (`confirm_external_completed` or
   `confirm_external_failed`) only after confirming that the prior worker
   is fenced and taking responsibility for all remaining step effects.
   This is an explicit human resolution, not inferred exactly-once delivery.
   The reconciled execution state, firing transition and immutable audit
   decision share one transaction.
6. The host passes a stop receiver to the Workflow scheduler. Host shutdown
   closes new admission and drains already accepted local executions. The
   existing outer forced-abort deadline still applies, and an aborted
   operation remains explicitly uncertain for the next instance.
7. Workflow steps that call external providers must eventually use provider-
   supported stable idempotency keys (or an owner receipt/reconciliation port)
   before any automatic restart of ambiguous steps can be enabled. No
   generic exactly-once guarantee is claimed.

## Invariants and data ownership

- Workflow tenant ID and workflow ID scope all firings; schedule revision and
  UTC scheduled instant define the stable occurrence.
- Cursor CAS and new firing rows share one transaction; a partially committed
  schedule page is impossible.
- The durable firing row is the canonical admission/attempt state; the
  `workflow_executions` row and individual step journal remain the execution
  facts. REST/GraphQL projections are not the ownership authority.
- No lease expiry alone permits an expired `running` attempt to be replayed.
- A per-firing attempt token fences stale completion writes, including races
  with recovery/record changes. A stolen lease must never authorize effects.
  Claim ownership, current schedule version and insertion of the first
  execution journal are verified in a transaction that locks the same workflow
  row as step-graph edits; external effects begin only after commit.
  A heartbeat refreshes active claims without canceling in-flight effects.
  The engine checks the attempt token before subsequent steps and commits its
  final execution status under the same firing-row CAS. A stale worker cannot
  overwrite an operator decision with a late status write.
- Workflow edits invalidate stale revision admission. The exact schedule
  revision is checked against the active workflow before dispatching a
  pending firing; queued old-revision firings are terminally skipped.
- Non-idempotent external steps with ambiguous outcomes require manual
  reconciliation. Existing `running` records must be preserved through
  migration and flagged for review, not reset.

## Additional owner-side admission and deletion guards

- A cron definition must be parseable by the runtime scheduler before a
  workflow enters `active`, is modified while active, or is restored while
  active. Incomplete draft definitions remain editable. Validate the
  expression at the owning write transaction boundary (bounded to 512 bytes).
- A scheduler stale-attempt scan must re-check both the attempt token and
  `claimed_at` age in its conditional update. A heartbeat that refreshes
  `claimed_at` after the scanner's initial read prevents quarantine.
  Time-based expiry still never authorizes a new execution.
- Each configured workflow step timeout bounds every attempt (1–3,600,000
  milliseconds). Deadline expiry is a confirmed timeout only for purely local
  steps; for non-transactional/external steps, cancellation does not prove
  the external system did not accept the request. Even error labels such as
  `InvalidStepConfig` from pluggable external executors are not proof of
  pre-effect rejection.
- A workflow hard-delete must be refused while its execution journal is
  running or reconciliation-required, or any cron firing is running or
  reconciliation-required. Pending firings that were **never admitted** may
  be cancelled by explicit workflow deletion. Historical terminal receipts
  remain subject to the existing separate data-retention contract; this
  proposal does not establish indefinite storage.
- The execution journal, not a transient engine return value, determines
  whether a cron firing has a durable terminal result. An error after the
  terminal journal write must not downgrade the firing to ambiguous.
- Schedule definitions advance their revision timestamps monotonically
  (minimum one microsecond beyond the previous committed definition revision).
  Changing only failure-count bookkeeping must not reset a cron cursor; a
  state transition such as automatic pause must invalidate older pending
  occurrences. When a replica's wall clock is behind a persisted future
  definition timestamp, new cursor seeding is capped at current UTC rather
  than deferring firing until that future timestamp.

## Failure and operational semantics

A provider failure before a known durable receipt is ambiguous even when the
provider returned a timeout. A DB failure after sending may leave the same
uncertainty. The scheduler must not silently convert either into safe retry.
Unique keys protect admission and records only. A multi-replica deployment may
have more than one process active; a DB CAS is the authority, not a process
mutex or timer.

## Migration and rollout

Add additive owner migrations, indexes, and bounded query APIs. Existing
`workflow_executions.running` records are not changed by migration. Apply
migrations before enabling the new cron scheduler; no fallback to the old
in-memory one-second firing path is permitted if the schema is missing.
Gradually enable new workers and observe queue depth, stale claims and
reconciliation-required counts. Operator reconciliation remains a gated
follow-up until its authenticated command, evidence and audit/outbox path are
approved.

## Verification required before acceptance

- SQLite and PostgreSQL migration tests, atomic cursor/enqueue rollback tests.
- Two-replica concurrent claim tests and safe restart of queued firings.
- Crash windows before claiming, after claiming but before execution, during
  an external effect, and after a terminal execution write.
- Schedule edits, invalid expressions, large downtime, clock skew, tenant
  isolation, bounded backfill and idempotent duplicate slot admission.
- Fencing tests for late completion and operator reconciliation audit.
- Host shutdown tests for no new firings and controlled drain.
- Operator-facing conflict/uncertainty classification and authorized review.
- Scoped Rust checks/tests on server and Workflow owner (maintainer-owned).

## Alternatives rejected

- Plain one-second polling: loses downtime and duplicates across processes.
- A lease-expiry replay queue: cannot fence an already-issued external effect.
- A process mutex or Redis lock without a durable receipt: cannot prove
  external completion or atomicity with the database.
- Re-running the Workflow engine from step zero after a crash: reissues
  nontransactional HTTP/notification effects without reconciliation evidence.
