# Installer HTTP job crash quarantine

- Date: 2026-11-24
- Decision status: Accepted
- Implementation status: In progress
- Owners: `apps/server`, `rustok-installer`, `rustok-installer-persistence`
- Extends: [Durable operations do not retry unresolved side effects](./2026-11-12-durable-side-effect-recovery.md) and [Module release rollback safety](./2026-08-06-module-release-rollback-safety.md)
- Supersedes: None
- Superseded by: None

## Context

The HTTP installer has two durable persistence boundaries. The host job ledger
is stored in the server runtime database so another server replica can observe
the submission. The install session and step receipts are stored in the
database selected by the install plan. Those databases may be different.

If the host process disappears after target-side effects begin but before the
host job receives a terminal write, the job row remains `running`. Lease
expiry alone cannot prove that the executor stopped, and the host ledger does
not contain a safe reconnectable reference to an arbitrary target database.
Resetting the row to `failed` or admitting a replacement attempt could
duplicate migrations, seed writes, administrator provisioning, or deployment
effects.

## Decision

1. **Heartbeat the host job.** The owning HTTP runtime refreshes
   `install_http_jobs.updated_at` every 30 seconds while the executor task is
   active. The heartbeat updates only a row whose status is still `running`.
2. **Quarantine stale jobs.** A compare-and-set moves a `running` row whose
   heartbeat is older than five minutes to `recovery_required`. The predicate
   includes the observed timestamp cutoff, so a concurrent heartbeat wins.
   The transition does not set `finished_at`: the host does not know whether
   the executor stopped or whether target-side effects completed.
3. **Keep the global admission fence closed.** A unique expression index maps
   both `running` and `recovery_required` to the same non-null key. The
   application admission query checks both states. A stale job is never
   reset to `failed` merely because its heartbeat or lease expired.
4. **Preserve late-worker fencing.** Heartbeats and terminal writes are
   conditional on `status = running`. A late heartbeat cannot revive a
   quarantined job; a late success/failure write cannot overwrite the
   recovery state. A worker that loses its heartbeat fence may still be
   finishing an already admitted target operation, but no new HTTP apply is
   admitted until operator reconciliation.
5. **Classify ambiguous executor outcomes.** Database preparation,
   owner-schema application, session creation/claim, receipt persistence,
   state transitions, and post-schema effects carry a typed recovery-required
   error classification. The host job is quarantined rather than reported as
   a clean failure. `FreshInstallCleaned` is not used unless cleanup actually
   ran. The state machine requires `Deploying` before `Verified` and does not
   allow new transitions into the legacy `failed` state.
6. **Expose the correct persistence boundary.** Global status projects the
   host job when the target session is not available through the runtime
   database. The receipts endpoint returns a conflict instead of an empty
   list when a host job exists but its target session is remote.

## Sources of truth and ownership

- The server runtime database owns the durable HTTP job ledger, its heartbeat,
  public status, and the single unresolved-job admission fence.
- The selected target database owns the install session, attempt lock,
  attempt generation, and step receipts.
- The host job status is a projection of executor liveness/outcome, not proof
  of target rollback or completion. Target session/receipt state remains
  authoritative for effects that reached the target.
- The HTTP host does not persist a raw target database URL or a reconnectable
  secret reference in the job row.

## Invariants

### Allowed states

- `running`: an executor has been admitted and its host heartbeat is fresh.
- `recovery_required`: the heartbeat expired or an executor outcome became
  ambiguous; no new HTTP apply is admitted.
- `succeeded` and `failed`: terminal host-job outcomes persisted by
  compare-and-set from `running`.

### Forbidden states

- A stale `running` row must not be reset to `failed` based only on elapsed
  time.
- More than one row may not be `running` or `recovery_required` at once.
- A late heartbeat or terminal write may not change
  `recovery_required` back to `running`, `succeeded`, or `failed`.
- An install session may not be marked `fresh_install_cleaned` unless the
  cleanup operation actually ran.
- Verification may not bypass the distribution deployment stage.

## Non-goals

- Automatic resume or replay of interrupted installation stages.
- Lease-expiry takeover or concurrent execution against an existing target
  session.
- Exactly-once delivery of external effects.
- Persisting arbitrary target credentials in the host job ledger.
- Automatic cross-database receipt reconciliation after process loss.

## Data, transaction, and concurrency boundary

- Heartbeats use an UPDATE constrained by job ID and `status = running`.
- Stale reconciliation uses one atomic UPDATE constrained by
  `status = running` and `updated_at <= stale_before`; a concurrent heartbeat
  wins if it advances the timestamp first.
- Success, failure, and recovery-required transitions compare-and-set from
  `running`.
- A unique expression index maps both unresolved statuses to one key. The
  migration creates the new index before dropping the old running-only index;
  if duplicate unresolved history prevents creation, the old guard remains.
- The host job ledger and target session/receipt database are not one
  distributed transaction. No cross-database atomicity is claimed.

## Context dimensions

- Principal/auth: installer setup-token authorization protects apply, job
  status, and receipt operations; the global status projection exposes only
  the existing public status fields plus the host job identifier/state.
- Tenant: tenant identity is derived from target-side install results and is
  not used as a host-job admission key.
- Locale, channel, currency, timezone and policy context: not applicable to
  this installer control-plane decision.

## Events and projections

- The global status response projects the latest host job when its target
  session is not present through the runtime database connection.
- The receipts endpoint returns a conflict when the host job exists but its
  target session is not available through the runtime connection; it does not
  project a missing target receipt set as an empty successful result.
- No event/outbox publication is added by this decision. The durable host job
  row is the status source of truth.

## Failure semantics

- Heartbeat database errors are logged and retried on the next heartbeat
  interval. If the host database becomes available only after the stale
  threshold, reconciliation conservatively quarantines the job.
- If a heartbeat finds that the row is no longer `running`, it stops
  heartbeating; it does not revive the row or cancel an arbitrary in-flight
  target operation.
- A process paused or disconnected long enough to miss the heartbeat deadline
  may be quarantined even if it later resumes. The old executor may still be
  finishing an already admitted operation, so operator reconciliation must
  first verify that the old executor is stopped.
- The job ledger does not contain a reconnectable target credential or
  reference. The server therefore cannot automatically open a remote target,
  compare its receipts, replay steps, or release its session lock after a
  crash. An operator must inspect the actual target database and receipts,
  verify that the former executor is stopped, and reconcile its effects before
  retrying.

## Migration and cutover

- Add a versioned migration that creates the cross-status unique expression
  index before dropping the prior running-only index.
- If existing unresolved rows violate the new global uniqueness invariant,
  migration fails closed and leaves the previous running-only guard in place.
  Operators must reconcile duplicate rows before retrying the migration.
- Existing `failed` host-job rows are retained as history; this migration
  does not rewrite them or infer target-side outcomes from them.
- Existing target session/receipt history is not deleted or reset.

## Alternatives considered

- **Reset stale jobs to `failed`:** rejected because elapsed time is not proof
  that target-side effects did not occur.
- **Take over the target session after lease expiry:** rejected because the
  previous process may still be alive and lease expiry does not fence external
  effects.
- **Store the raw target database URL in the host job:** rejected because it
  would persist arbitrary credentials in the runtime job ledger.
- **Index unresolved rows by `status` directly:** rejected because a unique
  index on status would allow one `running` and one `recovery_required` row.
- **Automatically replay from the last receipt:** rejected because the host
  cannot currently reconnect to arbitrary remote targets and receipts do not
  prove that every external effect was rolled back or committed exactly once.

## Verification

Source regressions cover state-transition invariants, typed error
classification, heartbeat compare-and-set, stale-job quarantine, recovery
finalization, status projection, and the cross-status single-slot migration.
Tests, compilation, formatting, migration execution, and multi-replica
exercises were not run at the maintainer's request.

## Consequences

- A crashed or ambiguous HTTP install is visible as `recovery_required`
  after the stale threshold, and it continues blocking new HTTP applies.
- The operator must inspect the target database and receipts before retrying;
  the server does not claim automatic target recovery.
- A conservative false-positive quarantine can reduce availability when a
  process is paused or a database is temporarily unreachable.
- The next architectural step for automatic target reconciliation requires a
  safe, host-owned target connection reference or an explicit operator-supplied
  target capability. It must be designed separately and must not weaken the
  current fail-closed admission fence.
