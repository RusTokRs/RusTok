# FORUM-20BQ — Search inbox startup and periodic sweeper

## Decision

FORUM-20BQ closes the idle-tenant recovery gap left by FORUM-20BP. The server now starts a host-owned background worker when runtime settings permit background work, a Forum projection source is registered, and PostgreSQL is available.

The host owns only lifecycle. Search continues to own due-work discovery, replay, projection, retries, dead letters, watermarks and advisory-lock serialization.

## Startup and periodic recovery

The worker performs a sweep immediately after startup, before its first sleep. It then repeats every five seconds until the shared `StopHandle` signals shutdown.

A worker handle stored in `ServerRuntimeContext` prevents duplicate startup in one process. Multiple server processes may run the worker because the existing tenant-wide Forum advisory transaction lock remains the cross-process execution owner.

The worker is not started when background workers are disabled, the Forum Search projection source is unavailable, or the database backend is not PostgreSQL.

## Due-tenant selection

Search selects at most 32 tenants per sweep. For each tenant it first identifies the oldest non-terminal Forum inbox row by `(revision_at, event_id)`.

A tenant is due only when that oldest row is pending or its retry backoff has expired. A newer due row therefore cannot bypass an older retryable row that is still in backoff.

Due tenants are ordered by their oldest revision. Each selected tenant processes at most 64 events per sweep. Public APIs reject zero or excessive limits; tenant and per-tenant event limits are capped at 256.

## Replay boundary

`ForumProjectionReconciler` reuses:

- `ForumProjectionInbox::claim_next`;
- the existing full and targeted Forum projectors;
- the existing Search and Blog projectors for tenant, locale and full Search rebuild events;
- the FORUM-20BP retry, dead-letter, watermark and advisory-lock semantics.

The server worker does not query `search_projection_inbox`, write watermarks, mutate inbox status or construct Search projection SQL.

## Failure behavior

A projection failure is persisted through the existing durable retry path. The current tenant stops processing for that sweep, while other due tenants remain eligible. Lock contention leaves work durable for the next pass.

A database-level sweep failure is logged by the host worker and retried on the next interval. It does not terminate the server process.

## Compatibility

This slice adds no migration, dependency, event, reindex target, route, GraphQL field, public DTO or Search query change. `Cargo.lock`, FFA and FBA status remain unchanged.

The existing envelope timestamp plus event-ID ordering remains in force. Owner-issued monotonic revisions for cross-producer clock-skew independence remain downstream work.

The large canonical Forum plan, Forum `CRATE_API.md` and Search local plan remain conflict-sensitive synchronization debt and are not rewritten through the GitHub Contents API.

## Remaining work

FORUM-20BR should address owner-issued monotonic revisions or continue FORUM-23 with safe author summaries, member projections and filter contracts. Maintainer-executed PostgreSQL evidence is still required for startup recovery, periodic idle recovery, multi-process contention, retries, crash replay and ordering.

## Validation status

No tests, Cargo commands, formatting, verifiers, workflows or CI were run by the implementation agent.
