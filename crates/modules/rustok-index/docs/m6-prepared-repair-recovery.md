# M6 prepared repair recovery policy

Status: `source_complete_owner_execution_pending`.

## Purpose

A durable targeted-repair command can remain in `prepared` after a process failure, source failure,
owner failure, after-evidence failure, or receipt-write failure. The command is intentionally
ambiguous: elapsed time does not prove whether the owner side effect happened.

This slice adds one internal fail-closed recovery policy. It distinguishes an active command from an
operator-paused or terminally abandoned command, preserves the original repair identity, and requires
an independently authorized operator decision before an ambiguous legacy command can resume.

It does not add a scanner, timeout, scheduler, public transport, automatic takeover, or orphan-link
mutation owner.

## Recovery command and authorization

`IndexDriftRepairRecoveryCommand` binds:

- exact tenant, finding, and durable repair command UUIDs;
- the original lowercase SHA-256 repair payload digest;
- one non-nil recovery decision UUID;
- the expected latest recovery revision, or `None` only for an unclassified command;
- one typed `Resume`, `Pause`, or `Abandon` action;
- one bounded lifecycle actor identity;
- one trimmed nonempty reason bounded to 512 bytes.

The payload digest is the existing domain-separated commitment over tenant, finding, repair command,
typed target, original actor, and original reason. A recovery caller cannot substitute another
target or original command identity.

`IndexDriftRepairRecoveryAuthorizer` runs before storage. No allow-all implementation or public
transport is provided. Only the recovery service constructs
`IndexDriftAuthorizedRepairRecoveryCommand`.

## State machine

The immutable recovery ledger admits only these transitions:

- `unclassified -> active` by initial `activate` or authorized `resume`;
- `active -> paused` by authorized `pause`;
- `paused -> active` by authorized `resume`;
- `unclassified | active | paused -> abandoned` by authorized `abandon`.

`abandoned` is terminal. It is the operator's terminal not-repaired recovery decision; it does not
fabricate before evidence, after evidence, an owner receipt, or a repair receipt.

A newly reserved command receives revision `0`, action `activate`, and state `active`. The initial
ledger row reuses the same command UUID as its deterministic decision UUID and copies the original
actor and reason.

A pre-existing `prepared` command with no recovery row is `unclassified`. Exact repair retry fails
with `index_drift_repair_recovery_required`; it is never auto-activated. An authorized `Resume` with
`expected_revision = None` is required.

## Immutable PostgreSQL store

`PostgresIndexDriftRepairRecoveryStore` uses one PostgreSQL `SERIALIZABLE READ WRITE` transaction and
the same tenant/command advisory-lock namespace as repair reservation and completion.

For each decision it:

1. locks the exact command identity;
2. verifies exact finding and payload digest;
3. rejects completed or malformed command state;
4. returns an exact repeated decision idempotently or rejects decision UUID payload drift;
5. compares the caller's expected revision with the latest durable revision;
6. rejects invalid transitions and resume after finding lifecycle closure;
7. appends one immutable decision row.

Migration `m20260806_000008_add_index_finding_repair_recovery` creates
`index_consistency_finding_repair_recovery_decisions`, unique command revisions, append-only
PostgreSQL/SQLite guards, and a repair-completion guard.

No decision is derived from `created_at`, wall-clock age, lease expiry, or process liveness.

## Repair admission and owner fence

`RecoveryAwareIndexDriftRepairStore` is composed into the existing concrete missing-entity service.
It rejects exact retry when the latest state is missing, paused, or abandoned. New reservations are
activated only after the durable `prepared` row exists; if activation persistence fails, the command
remains unclassified and therefore fail-closed.

`RecoveryAwareIndexDriftRepairOwner` acquires the same command advisory lock, validates the exact
stored finding and payload digest, and requires latest state `active` before delegating to the
idempotent missing-entity owner. The lock remains held through the owner call.

Therefore a pause or abandon either:

- commits before owner admission and blocks the side effect; or
- waits for the already admitted owner call to finish.

It is not a cancellation signal for an owner call that already acquired the fence.

## Completion and crash windows

The recovery-aware store requires latest state `active` before delegating completion. The migration
also installs a database trigger on the original repair-command table, so `prepared -> completed`
cannot commit unless the latest recovery row is still `active` at the database transition.

An operator decision can win after an owner side effect but before receipt persistence. In that case
completion fails closed. The implementation does not infer the side-effect result. An authorized
resume of the same command preserves the original mutation UUID and reaches the existing inbox
idempotency path before admitted after evidence and receipt completion.

## Finding lifecycle coordination

Authorized resume requires the exact finding to remain open. Pause and abandon may still be retained
for a prepared command after lifecycle closure so the ambiguous repair history is not silently lost.
The repair service continues to leave finding state unchanged; recovery does not resolve, ignore,
reopen, or delete a finding.

Original repair identity remains in `index_consistency_finding_repair_commands`, and every recovery
decision is append-only. Recovery never rewrites the original target kind, actor, reason, payload
digest, or terminal repair receipt.

## Privacy and transport

Command `Debug` output exposes actor-subject and reason lengths only. Public failures contain bounded
machine codes. The PostgreSQL recovery store and materializer are crate exports, but no GraphQL,
HTTP, CLI, MCP, native-admin, runtime-extension, scheduler, worker, or automatic iteration surface is
registered.

## Deliberate limits

This slice does not add:

- time-based lease expiry or automatic ownership inference;
- cancellation of an owner call after it has acquired the command fence;
- a fabricated `NotRepaired` repair receipt without admitted evidence;
- automatic finding lifecycle transitions;
- orphan-link mutation repair;
- retained migration, PostgreSQL/SQLite, concurrency, crash-window, workflow, or CI evidence.

## Next implementation step

Compose one concrete orphan-link evidence and mutation owner behind the existing recovery-aware
repair boundary. Preserve exact link identity, ordinal, target absence proof, command UUID
idempotency, and the same active recovery fence.

Keep public transport, automatic finding iteration, and production evidence admission separate.

## Suggested maintainer validation

```bash
cargo test -p rustok-index drift_repair -- --nocapture
cargo test -p rustok-index drift_missing_entity_repair -- --nocapture
node scripts/verify/verify-index-prepared-repair-recovery.mjs
node scripts/verify/verify-index-targeted-drift-repair.mjs
node scripts/verify/verify-index-missing-entity-repair-composition.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
git diff --check
```

No tests, Node verifiers, formatting, Cargo checks, migrations, PostgreSQL/SQLite scenarios,
workflows, or CI were executed by the implementation agent.
