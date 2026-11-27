# Invalidated RBAC request snapshots fail closed

- Date: 2026-11-02
- Decision status: Accepted
- Implementation status: Implemented
- Owners: apps/server (request scope), rustok-rbac (persisted grants and policy)
- Extends: 2026-02-26-rbac-relation-source-of-truth-cutover
- Supersedes: None
- Superseded by: None

## Context

An authenticated request can carry permissions restricted by OAuth scopes. A
same-actor RBAC mutation invalidates the request snapshot, but falling back to
persisted grants can drop the token's narrower ceiling. The role snapshot can
also no longer be trusted. Permissions use implied `manage` semantics, so a
naive intersection of permission vectors is not a sound general solution.

## Decision

For a matching tenant and actor whose task-local request snapshot was invalidated,
the server's scope-aware permission decisions, permission listing and role lookup
must report unavailable authority, not use the old snapshot or fall back to
unscoped persisted grants. No matching snapshot (including another actor or tenant)
continues to use the existing persisted-grant behavior. Existing guards requiring
a valid snapshot continue to reject an invalidated one. In the server GraphQL
schema, a root-field admission hook rejects a new root resolver when the
matching authenticated task-local snapshot is invalid. Nested fields of a root
resolver already admitted are not rechecked by this hook. After a committed
grant edit the server's generation-aware role-writer clears the local cache and
conservatively expires the calling task's snapshot, including for an unrelated
role. Its retained legacy membership hook expires the calling snapshot before its
fallible lookup. The later
[grant-invalidation decision](2026-11-02-rbac-graphql-grant-invalidation.md)
records the durable generation and fan-out contract.

An authenticated request can be retried with fresh authentication context where
appropriate; callers must not blindly replay a mutation that may already have
committed. Returning unavailable is intentionally different from returning an
empty permission set or permission denied. Invalidation is terminal for that
snapshot; it is not refreshed or widened within the same task-local scope.

## Sources of truth and ownership

`rustok-rbac` owns persisted relation grants and policy. The server owns the
request-effective snapshot and its invalidation behavior. Token scopes and the
request's effective authority are ceilings, not new persisted grants.

## Invariants

### Allowed states

A valid matching snapshot can be used for request checks; no matching scope can
resolve persisted grants under the existing caller contract. A matching invalid
snapshot yields an error on the scope-aware fallback paths.

### Forbidden states

The server must not reinterpret an invalid matching snapshot as absence of a
scope and use unscoped persisted grants, return its stale role, or turn the
unavailable condition into a successful authorization decision.

## Authenticated credential deadline extension (2026-11-27)

A credential-bearing scope also becomes terminally invalid when its original
exclusive deadline is reached or the shared Auth application clock cannot
produce a healthy local observation. Permission, role and credential accessors,
matching-scope fallback guards, GraphQL root admission and captured artifact
scopes share that check. The grant-only trusted internal constructor retains
its existing contract. This is a process-local time check, not durable clock
corroboration or a new session/grant revocation fence. Source regressions are
unrun.

## Process-local epoch binding (2026-11-27)

Authentication samples the existing bounded cache epoch for the exact tenant
and actor before resolving database authority and checks it again before
returning the admitted user. The original token is carried into HTTP and WS
request scopes, including captured artifact scopes. A matching scope rejects
epoch drift or exhaustion, even when invalidation occurred before entering
the task-local scope. No refresh of the original token is allowed. WebSocket
lease admission and local output checks also reject drift.

Targeted cache invalidation advances one of the existing 64 bounded stripes;
a collision conservatively expires unrelated authenticated scopes on that
stripe. A full clear advances the global epoch. Trusted grant-only scopes
retain their task-local contract. Cross-task observation requires no new
cache, subscription, allocation per actor or persistent generation. This
also observes remote invalidations after local delivery, not at remote
commit. Source regressions are unrun.

## Durable static authority at asynchronous admission (2026-11-27)

Retain the exact single-statement owner membership/role-grant revision
observed during JWT admission in CurrentUser and the authenticated scope.
Scope-aware service decisions, permission/role listing, the request policy
resolver and GraphQL root admission re-read that revision before consuming the
original ceiling. A mismatch, missing evidence or failed read invalidates the
shared scope; the local deadline and epoch are checked again after the read.
WebSocket revalidation compares the original revision even if the effective
permissions and role return to their original values. No new grants are loaded
into the request ceiling, no locks or migrations are introduced.

This closes direct static relation ABA at those asynchronous boundaries without
depending on Redis delivery. It costs one owner read per successful static
scope check. Service OAuth has no static membership revision; credential,
module availability, sync-only snapshot consumers, nested GraphQL fields and
already admitted effects require their respective authority contracts. An
ordinary read is not a commit-time fence. Source regressions are unrun.

## Fallible snapshot lookup and REST permission admission (2026-11-27)

Service and request-policy fallback consumers use a fallible snapshot lookup:
only actual absence or a different tenant/actor permits persisted fallback.
Matching invalidation remains an unavailable error even if it occurs while
cloning the original snapshot. The synchronous REST permission helper and
its extractor macro verify the CurrentUser's original exclusive deadline
and process-local epoch; a matching scope must also authorize each effective
permission. This preserves wildcard semantics without widening either
ceiling. Unavailable evidence is distinct from normal permission denial.
These synchronous checks perform no durable read or later effect fence.
Source regressions are unrun.

## GraphQL executor context binding (2026-11-27)

HTTP and WS controllers attach the original RbacRequestScope to executor
data. The security extension treats that marker as mandatory evidence:
task-local scope must equal the original scope, and AuthContext must not
exceed its effective permission ceiling. Credential-bearing markers also
bind the session, principal kind, OAuth client and token scopes. Missing or
substituted scope is unavailable; trusted unmarked schema calls retain their
existing contract. Root admission checks the binding before and after its
durable revision observation. Nested query fields check local validity,
including binding, without a database observation per scalar. Only the
selected mutation retains the nested receipt serialization exception.
Fresh protected reads within a mutation payload and durable revocation
during nested queries still require owner-specific checks. Source regressions
are unrun.

## Fresh nested reads versus command receipts (2026-11-27)

The shared GraphQL read authority handle is host-composed and fail closed:
missing provider, missing admission marker, wrong tenant, invalid scope or
durable static revision drift prevents a fresh protected read. Explicit
consumers check before reading and again before releasing the result. The
host reuses the original scope binding and durable static reader, not
unscoped persisted grants. This is an observation boundary, not a
transaction fence or continuous credential/module policy authority.

Blog moderation queue reads require effective Comments moderate AND list
grants (manage implies both), retain the actual actor and permission ceiling
instead of System, and recheck Blog module availability. User live role,
can, tenant name and custom fields reads require current authority.
The owner mutation record already carries role and tenant name; these two
receipt fields are serialized directly without a new read. A denied fresh
non-null field can still propagate a GraphQL null after commit. Other
receipt projections and protected nested consumers remain open. No schema
field type change. Source regressions authored, unrun.

## Durable request credential observations (2026-11-27)

Matching credential-bearing scopes now revalidate the admitted identity
against live tenant, human account, recovery revision and session (including
the absolute deadline), or OAuth app, grant, exact access registration,
scopes and consent lineage. Initial and subsequent OAuth access checks
share the same registration storage predicate; the scope retains only the
admitted digest, never the bearer token. Service principals also verify
that current app grants restricted by the original scopes still cover
every admitted permission; human principals retain the static revision
check. Loss of verification already observed at admission denies. Missing
storage/evidence or a failed observation terminally invalidates that scope,
including captured clones. Existing grant-only trusted scopes are unchanged.

These are observations at async service, GraphQL root and explicit fresh
read boundaries, not a single atomic credential/permission snapshot or
transaction fence. The guarded local clock is used at each check; no new
durable clock write is performed there. JWT key/witness revocation, newly
enabled verification requirements, continuous module policy and unscoped or
synchronous-only consumers require separate runtime authority integration.
A revocation restored before any observation is not necessarily detected.
Source regressions authored, unrun.

## Non-goals

This change does not create an atomic revocation fence, ensure immediate
cross-replica invalidation, propagate task-locals to spawned tasks, or make all
consumers of immutable `AuthContext` snapshots react to local invalidation. The
GraphQL hook cannot revoke a field already admitted; nested mutation payloads
retain the receipt exception. Absent task-local scope is invalid for a marked
HTTP/WS admission, not for trusted unmarked schema calls. Those other paths need
separate review and changes before claiming request-wide revocation or universal
ceiling enforcement. Authentication and explicit
transaction-bound authoritative reads retain their existing distinct contracts.

## Data, transaction, and concurrency boundary

No persistence or migration changes. The direct task-local invalidation helpers
retain exact tenant/actor or full-scope semantics; runtime cache invalidation
also advances the shared epoch as described above. This is not an atomic guard
over future commits or responses.

## Context dimensions

Tenant and actor identity select the snapshot. OAuth scopes restrict the initial
permissions. Channel, locale and trace fields do not change this lookup.

## Events and projections

No events are added. Existing invalidation events and durable generation retain
their contracts. Other authenticated tasks observe the process-local epoch at subsequent
scope checks. Replicas depend on existing event delivery and durable recovery.

## Failure semantics

A matching invalid scope yields the server's generic internal/unavailable error;
no stale or widened authorization decision is produced on these service paths.
The mutation triggering invalidation may already have committed.

## Migration and cutover

No stored data changes. Callers must tolerate an error on later matching checks
within an invalidated scope. Outside that scope, behavior is unchanged.

## Alternatives considered

- Re-resolve full persisted grants: rejected because it loses token restrictions.
- Intersect vectors: rejected without a canonical effective-permission and role
  ceiling model, since `manage` implies other actions.
- Return empty grants: rejected because unavailability is not a normal denial.
- Keep using the old snapshot: rejected because grants or roles may be revoked.

## Verification

Test valid, absent, mismatched and invalidated scopes, including single/any/all,
permission-list and role lookups, the owner-policy adapter, self-invalidation,
GraphQL root-field sequencing and post-commit membership-lookup failure, and
safe error mapping. Run server tests with the workspace toolchain and real
transport/database integration; local task-scope tests are not evidence of
cross-replica or transaction-spanning revocation.

## Consequences

Within an invalidated task-local scope, further scope-aware checks for that actor
fail. A new authenticated scope may be established at an appropriate boundary.
This availability tradeoff prevents the targeted same-task ceiling fallback;
other snapshot consumers remain an open security boundary.
