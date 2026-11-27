# Artifact Security State Boundary

Status: Accepted

Date: 2026-07-22

## Context

Registry yanking, tenant enablement, quarantine, and emergency revocation have
different meanings. Treating a yank as an execution disable would silently
rewrite tenant intent; treating an emergency compromise as ordinary discovery
metadata would leave an admitted artifact executable.

## Decision

`rustok-modules` owns a global security aggregate keyed by the immutable
artifact release tuple `(slug, version, payload_digest)`. Its state is one of
`clear`, `quarantined`, or terminal `revoked`.

`ModuleControlPlane::artifact_security` exposes three separately authorized
commands:

- `quarantine`: clear -> quarantined;
- `clear_quarantine`: quarantined -> clear;
- `revoke`: clear or quarantined -> revoked.

Every command requires an actor, policy revision, reason code/detail,
idempotency key, and expected security revision. State and a redacted outbox
event commit atomically. Exact completed receipts are stored for replay.
Quarantine can be cleared only by its explicit command; revoked releases never
return to clear. These transitions never update tenant module enablement rows.

Registry status is read separately. `active` and unlisted external releases
remain eligible for this security check, `yanked` is retained as a discovery /
install fact rather than an emergency stop. Registry mutations invalidate prior
execution leases without denying fresh admission solely because of a yank;
unknown registry status fails closed. Quarantine and revocation block new execution
through `ModuleEffectivePolicy`; the policy revision includes the security
snapshot but no grant contents or resolver error text.

## Execution effect serialization (2026-11-27)

Quarantine, clear and revoke acquire the permanent platform slug activation
lock before reading or writing the security aggregate, in a read-committed
owner transaction. SQL effect admission holds the same lock and checks the
lease's exact clear security revision after the lock wait. This protects the
implicit clear revision zero even before a security-state row exists. Clearing
quarantine advances the revision, never restoring prior execution authority.
The emergency writer does not consult the lifecycle journal mutation guard.
The lock covers short SQL transactions, not guest execution or external calls;
a security action waits for an already-admitted SQL transaction to finish.
Registry status has a separate monotonic release authority witness (migration
000096), and registry publication/yank/unyank share the same platform slug fence.
Serving SQL effects check both security and registry authority for every artifact
in the selected closure. See the execution capability lease ADR for rollout and
lock ordering. Old writers and leases must be drained before rollout.

## Consequences

- Emergency disable has a durable, revisioned, owner-owned path independent of
  marketplace discovery and tenant intent.
- Install/runtime policy can explain whether a release was yanked, quarantined,
  or revoked without conflating those states.
- A security-state row is global to the release digest, so every tenant and
  platform installation observes the same emergency decision.
- Maintenance/channel and node readiness remain separate Phase 8 inputs and
  must be added to the same effective-policy evidence before universal routing
  adoption.
