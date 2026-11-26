# Direct Session Refresh History

- Date: 2026-11-25
- Decision status: Accepted
- Implementation status: In progress
- Owners: platform / auth
- Extends: [Auth lifecycle invalidation](./2026-02-26-auth-lifecycle-unification-session-invalidation.md)
- Supersedes: None
- Superseded by: None

## Context

Direct first-party sessions retained only the immediately preceding refresh
digest. A second rotation erased evidence of the first, so a stolen older
credential could not trigger lineage revocation.

## Decision

Persist each outgoing refresh digest and rotation timestamp in
`session_refresh_history` in the same transaction as replacement. Every
recorded digest remains invalid for refresh. Replay of a recorded digest after
the 30-second per-token grace revokes the live session. A replay inside grace
returns an explicit already-rotated conflict without revoking the live session.
Host BFFs must preserve cookies on this conflict and mark the unforwarded
operation retryable: clearing cookies can race with the successful response
from the replica that won rotation. Missing or future rotation timestamps
never grant grace. The current-token and previous-cookie
logout contracts and absolute 90-day lifetime remain unchanged.

## Sources of truth and ownership

Auth owns the semantics; the server owns the session persistence adapter and
platform migrations install the table. The session row owns tenant, user,
expiry and revocation. History owns rotated credential evidence; previous-token
columns are the last-rotation projection used by logout, not replay authority.

## Invariants

### Allowed states

A session may have zero or more immutable historical digests. A historical
digest cannot issue credentials. Within grace its replay fails without revoking
a live session; outside grace it revokes that session.

### Forbidden states

No committed rotation may lose the outgoing digest. No historical digest may
revoke a different tenant's session. No history may outlive its parent session.

## Non-goals

OAuth token families, logout with older-than-previous cookies, and public API
shape changes are outside this decision.

## Data, transaction, and concurrency boundary

The history digest is a unique primary key; session ID is a foreign key with
delete cascade and a cleanup index. Tenant authority is derived through the
parent session. The current-session row lock serializes rotation and revocation.
History insertion and replacement commit together or both roll back. Replay
locks the same session by ID and rechecks liveness before revocation.

## Context dimensions

Tenant and direct-session identity determine authority. Locale, channel,
currency, timezone and presentation state do not alter credential validity.

## Events and projections

No new domain event is introduced. Session revocation remains durable evidence;
existing access-token session fencing observes it. The previous digest remains
a bounded projection for logout.

## Failure semantics

Unknown historical digests and all historical refresh attempts fail closed.
Storage failures propagate, and no successful refresh is reported if the
history write fails.

## Migration and cutover

Stop old host writers, apply the additive migration, and deploy the new host.
Backfill the exact last available digest and timestamp; older overwritten
digests cannot be recovered. Duplicate historical digests require explicit
reconciliation rather than silently discarding evidence. Rollback drops history
and must not be used while new-version sessions remain active. Session cleanup,
including the CLI, removes history via the foreign-key cascade.

## Alternatives considered

Keeping a fixed-size array loses older theft evidence. A separate denylist
with an independent expiry duplicates session lifetime and cleanup semantics.
The session-owned table retains every recorded generation without those gaps.

## Verification

SQLite and PostgreSQL backfill and cascade, actual multiple rotations, tenant
isolation, grace boundaries, failed insertion rollback, concurrent refresh
and replay, and host access-token revocation must be verified. Source tests
and fixture are authored; runtime verification remains pending.

## Consequences

One durable insert per successful refresh and indexed storage proportional to
rotations over retained sessions. No plaintext credentials are stored.
