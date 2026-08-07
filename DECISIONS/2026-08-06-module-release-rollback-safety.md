# Module release rollback safety

- Date: 2026-08-06
- Status: Accepted

## Context

RusToK needs an operator-friendly production module update experience: users
must be able to identify the selected and previous releases, start an update,
understand a failed rollout, and return to a verified predecessor when that is
safe. The platform is a compiled, distributed application, not a plugin
directory whose files can be overwritten in place.

Existing module-control-plane contracts already retain immutable artifact and
static-distribution release identities, predecessor lineage, audit/outbox
facts, migration checkpoints, artifact-data snapshots, and desired/observed
native rollout state. They do not justify automatic restoration of committed
database data. A source, sandbox, build, or rollout check alone cannot prove
that an old release remains correct after an irreversible data change.

## Decision

Production versioning and rollback are one owner-controlled lifecycle. A
production release is an immutable identity binding source, dependency lock,
build and test evidence, artifact digest, policy/admission facts, and executor
facts. Update and rollback are audited transitions between such identities;
they never replace source or artifact bytes.

`rustok-modules` is the sole owner of release selection, predecessor retention,
update preflight, rollback, incident receipts, and desired-versus-observed
rollout state. Hosts, UI adapters, CLI adapters, Alloy, sandboxes, deployment
agents, and module owners consume this owner contract and do not create a
second release ledger or rollback path.

An update has one operator-visible mode:

- **Automatic** is allowed only when the exact direct predecessor and its
  enabled dependency closure remain admitted, unrevoked, retained, and
  compatible with the expanded schema and live data. During a bounded
  observation window, deterministic startup, readiness, rollout-deadline, and
  policy-threshold failures may initiate one audited rollback to that
  predecessor. A failed return does not retry or oscillate; it produces a
  controlled stopped or degraded outcome.
- **Maintenance** is required for unproven predecessor compatibility,
  compensation, irreversible conversion, destructive cleanup, or incompatible
  schema change. Automatic rollback is not available after a compensating or
  irreversible checkpoint begins.

Migration policy remains an independent owner declaration with the existing
`reversible`, `compensating`, and `prohibited` values. It is not sufficient on
its own to authorize automatic rollback: predecessor and dependency-closure
compatibility must also be proven.

The normal data strategy is forward-compatible `expand -> migrate -> contract`.
Backward-compatible expansion and resumable, idempotent backfill may occur
before and during the automatic rollback window. Destructive cleanup,
irreversible conversion, and incompatible constraints are later finalization
steps after that window closes. A temporary compatibility bridge requires a
named owner and removal condition in the affected module plan.

Database restoration is a separately authorized recovery operation. It never
automatically overwrites live production data, because writes after a snapshot
would be lost. Module-scoped recovery is available only for explicit module
data ownership boundaries with a tested consistent restore procedure; otherwise
the recovery boundary is platform-level PostgreSQL recovery.

The neutral `rustok-sandbox` is an evidence and preflight layer for dynamic
artifacts. Alloy may use it for authoring and draft testing, but neither Alloy
nor the sandbox owns production release activation, rollback, or database
migration. Static/native compositions use their existing isolated build/test
and rollout evidence rather than claiming WebAssembly sandbox isolation.

Next.js build and deployment remain optional manual host operations and are
outside this automatic rollback lifecycle.

## Consequences

- The shared operator projection exposes selected release, predecessor,
  candidate, update mode, rollout outcome, rollback eligibility, and incident
  correlation identity. Its commands are update, observe, manual rollback, and
  disable/stop; it exposes no direct release-pointer, artifact-byte,
  registry-mutation, or database-restore operation.
- Every stateful module records runtime kind, data boundary, migration policy,
  update mode, predecessor and dependency-closure compatibility evidence,
  snapshot/recovery boundary, backfill behavior, rollback-window close
  condition, and verification in its existing implementation plan.
- Modules without this evidence fail closed into maintenance mode. Stateless
  modules may enter automatic mode once the central owner verifies their
  release and dependency facts.
- Automatic mode requires tests for `N -> N+1 -> N`, rejected candidates,
  failed readiness, revoked predecessors, duplicate commands, failed
  predecessor return, and rollout timeout. Maintenance mode requires proof
  that automatic rollback is denied and a recovery-required incident is
  recorded after its irreversible checkpoint.

## Related Documents

- [Module Release and Rollback Plan](../docs/modules/module-release-rollback-plan.md)
- [Module artifact rollback boundary](./2026-07-13-module-artifact-rollback-boundary.md)
- [Durable artifact-data snapshot and guarded restore](./2026-07-22-artifact-data-snapshot-restore.md)
- [Neutral sandbox foundation](./2026-07-11-neutral-sandbox-foundation.md)
