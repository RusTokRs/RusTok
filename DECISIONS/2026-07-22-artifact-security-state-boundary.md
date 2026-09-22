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
install fact but does not stop an already admitted execution, and unknown
registry status fails closed. Quarantine and revocation block new execution
through `ModuleEffectivePolicy`; the policy revision includes the security
snapshot but no grant contents or resolver error text.

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
