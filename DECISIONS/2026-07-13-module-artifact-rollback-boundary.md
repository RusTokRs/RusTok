# Module artifact rollback boundary

- Date: 2026-07-13
- Status: Accepted

## Context

An admitted module artifact is immutable and runs only from platform CAS. A
rollback must restore a prior admitted release without mutating its bytes or
changing the failed release's identity. The operation spans installation state,
capability grants, tenant scope, audit, and asynchronous runtime convergence.

## Decision

`rustok-modules` owns a typed rollback command. It requires the installation
scope, current installation identifier and admission revision, actor,
non-empty reason, and idempotency key. The command resolves only the durable
previous-installation pointer; it never consults an external OCI registry.

Within one database transaction the owner verifies the requested revision and
scope, re-evaluates capability grants for the target immutable release, writes
the guarded lifecycle revisions, records the rollback audit fact, and enqueues
one `module.artifact.rolled_back` outbox event. A stale revision, missing
predecessor, unavailable capability evaluator, or prohibited migration policy
rejects the command without a partial state change.

Rollback changes the selected admitted release and its lifecycle state. Runtime
activation and tenant enablement are separate reconciled operations; neither is
implicitly changed by the rollback command.

## Consequences

The installation predecessor link is durable rollback history and must remain
retained by CAS/GC policy. The command needs a module-owned lifecycle store,
capability-grant evaluator, audit record, idempotency record, and outbox event
in addition to admission persistence. Hosts may expose transport adapters but
must not update artifact admissions directly.
