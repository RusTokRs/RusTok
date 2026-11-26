# Forum deletion revalidates tenant policy under the write fence

- Date: 2026-11-26
- Decision status: Accepted
- Implementation status: In progress
- Owners: rustok-forum
- Extends: [Author self-service and deletion](./2026-10-09-forum-author-self-service-and-deletion.md)
- Supersedes: The preflight-only policy ordering in the extended decision; ownership, scopes, defaults and snapshot reasons are unchanged
- Superseded by: [Transactional settings capability](./2026-11-26-forum-transactional-settings-capability.md) for the missing-reader fallback only; all other rules remain active

## Context

Topic and reply deletion authorize ownership and `allow_user_content_deletion`
through a static reader before starting their transaction. A policy revocation
observed after waiting for a write fence cannot stop an already admitted delete.
The earlier decision explicitly accepted that ordering. It is inconsistent with
the transactional policy admission now used by content, slug and quote edits.

## Decision

Retain preflight for early refusal, but do not treat it as authority to delete.
After acquiring the canonical source write fence, recheck ownership and resolve
`allow_user_content_deletion` through the transactional settings reader. Determine
the persisted delete-snapshot reason from that admitted security/policy context.
Only then capture snapshots, delete source/solution state, update aggregates and
publish the existing events. Failed policy reads propagate; missing transactional
settings use the existing default false and never grant author deletion.

## Sources of truth and ownership

Forum topic/reply owners enforce the policy. The host settings capability supplies
the tenant's Forum settings in the caller's transaction. Core permission scopes
remain authoritative for Own/All. REST, GraphQL and both UI implementations continue
to delegate to these owners without new settings or client-specific rules.

## Invariants

### Allowed states

An author with Own delete scope deletes only their own source when both checks
admit it. All-scoped staff/system deletion retains its current policy exemption and
moderation/system reason. Existing defaults and typed errors remain unchanged.

### Forbidden states

- A preflight allow bypassing a later transactional deny or failed settings read.
- Stale source ownership admitting a delete after the lock wait.
- Snapshot, revision, aggregate or outbox changes surviving failed admission.
- Applying author self-deletion policy to an already-authorized moderation effect.

## Non-goals

Changing role permissions, session revocation, author edit windows, deletion UI,
restore semantics, or the platform settings writer's isolation/locking contract.
No claim that an ordinary transactional read alone blocks every later settings write.

## Data, transaction, and concurrency boundary

Topic deletion rechecks after its existing category/topic locks and source reload.
Reply deletion shares category -> topic -> solution -> reply locking between its
public command and the internal moderation removal path. A private locked-removal
core keeps snapshot/content/aggregate effects identical without moving authorization
into the internal moderation API. The public command performs admission between
locking and that core. All destructive effects, reasons and events commit together.

## Context dimensions

Tenant and source owner are read under the command fence. Principal/action scopes
come from the request's security snapshot; this decision does not refresh a session.
Locale, channel, currency and display timezone do not alter self-deletion policy.
Trace context and existing request adapters are unchanged.

## Events and projections

No new events or projections. Refused admission emits no success event or search
invalidation. Successful deletion preserves existing atomic topic/reply status
changes and category/topic invalidations.

## Failure semantics

Author policy denial returns Forbidden. A missing transactional reader defaults to
false; a settings capability error propagates and rolls back. Staff remain exempt
from a false author policy, but a settings capability failure is not silently swallowed.
The existing internal moderation removal API retains its caller-owned authorization.

## Migration and cutover

No schema migration or data rewrite. Existing paired settings readers in REST and
GraphQL runtimes are reused. Hosts supplying only a static allow now fail closed
for author deletion rather than bypassing transactional policy. No legacy path.

## Alternatives considered

- Keep preflight-only ordering: lets deletion ignore a policy revocation seen under
  its write fence.
- Duplicate the entire reply-removal implementation: lets tombstones, accepted
  solutions, counters and events drift between direct deletion and moderation.
- Impose author policy on internal moderation effects: changes authority semantics
  and may prevent authorized moderation from removing prohibited content.

## Verification

Shared SQLite/PostgreSQL service matrices must distinguish static and transactional
readers for topic and reply deletion. Test allow -> deny, allow -> failure, missing
transactional reader, successful author deletion, staff exemption, system reason,
foreign ownership and early preflight refusal. Compare exact source/body/revision/
solution/statistics state, snapshot counts and outbox counts on rejection. Positive
cases assert the saved user_request/moderation/system reason. Rust execution and
real settings-writer contention remain required; source guards are supplemental.

## Consequences

Each direct deletion adds a transactional settings read and ownership recheck.
It reuses existing source locks and preserves one removal effects implementation.
A stricter policy observed under the write fence can now reject a previously
admitted request instead of destructively following stale policy.
