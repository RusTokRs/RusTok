# RBAC cache-resolution consistency failures

- Date: 2026-11-02
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-rbac, apps/server (adapter)
- Extends: 2026-02-26-rbac-relation-source-of-truth-cutover
- Supersedes: None
- Superseded by: None

## Context

A generation-aware permission cache can reject a relation read when invalidation
races with the read. Returning the last rejected read after bounded retries may
use a revoked grant. Returning an empty successful set instead would disguise a
transient inability to resolve authority as a legitimate permission denial.

## Decision

The RBAC owner returns a typed resolution error when every bounded publication
attempt is rejected. Store errors remain distinct and retain their original
cause. Callers must propagate failure without producing an authorization decision
or request permission snapshot; they may retry at an appropriate boundary.
Adapters map the typed failure to a safe unavailable/internal error, not an
allow, a fabricated empty grant set, or a normal permission-denied decision.

## Sources of truth and ownership

`rustok-rbac` owns resolution and the typed error; persisted tenant-scoped role
relations remain the source of grants. Cache generations and request snapshots
are derived state. `apps/server` owns its cache adapter and transport mapping.

## Invariants

### Allowed states

A cache hit, or a relation read accepted by the adapter's conditional publication,
can be returned. Store failure or exhausted generation checks produce an error.
The nil-identity fast path retains its existing empty-denial behavior.

### Forbidden states

A read rejected by every conditional publication attempt must not be returned as
current or converted into a successful permission set. Neither the owner nor an
adapter may silently turn that error into an allowed decision.

## Non-goals

This decision does not establish an atomic revocation fence at a later write or
response boundary, coherent snapshots across multiple relation queries, immediate
cross-replica invalidation, or guarantees for adapters that use the default
non-generation-aware cache methods. It does not replace request-scope ceilings.

## Data, transaction, and concurrency boundary

No schema or persistence migration is required. The generation comparison is a
local cache-fill check, not a database transaction or a fleet-wide fence. Existing
bounded retries remain; a failed attempt's relation read is discarded.

## Context dimensions

Resolution is bound to tenant and user IDs. Channel, locale, timezone and currency
are not inputs to this cache check. Authentication, trace context and the policy
engine remain the responsibility of their existing owners.

## Events and projections

Existing committed RBAC invalidation and outbox ownership remain unchanged. No
new event is emitted. Cached grants are projections and never the authority.

## Failure semantics

Cache-generation contention is a typed unavailable condition distinct from a
stored empty permission set; store errors remain separately distinguishable.
Client-facing errors must not expose cache keys, tenant IDs or internal state.

## Migration and cutover

The exported cached-resolution result and runtime-adapter conversion bound change;
all in-tree adapters must migrate in the same change. External Rust consumers must
handle the new error. No stored data requires migration.

## Alternatives considered

- Return the last rejected read: rejected because invalidation can reflect revocation.
- Return an empty successful grant set: rejected because it misrepresents an
  infrastructure consistency failure as a permission decision.
- Retry indefinitely: rejected because invalidation churn can make requests unbounded.

## Verification

Owner tests must cover rejected reads, successful bounded retry, store errors,
cache hits and authorization error propagation. Host adapter tests must check
safe error mapping. Run the targeted owner/server tests with the workspace
compiler, plus concurrent integration tests with a real database and invalidator.
These checks do not prove transaction-spanning or multi-replica revocation fences.

## Consequences

Sustained local invalidation can temporarily make authorization unavailable rather
than grant from an unvalidated read. Operators need actionable contention telemetry;
full runtime, PostgreSQL and distributed evidence remain release gates.
