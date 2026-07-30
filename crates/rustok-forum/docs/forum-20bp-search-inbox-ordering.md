# FORUM-20BP — Search inbox ordering and reconciliation

## Decision

FORUM-20BP adds Search-owned durable delivery state for Forum projection work. It does not add a second domain event log and does not change Forum event payloads. Search stores one replayable consumer row per existing platform event ID.

The consumer revision is the existing envelope pair:

1. UTC `EventEnvelope.timestamp`;
2. ULID-backed UUID `EventEnvelope.id` as the deterministic tie breaker.

This is a consumer ordering key, not an owner-issued causal revision. Correct ordering across independent producers still assumes reasonably synchronized clocks.

## Durable owner

The Search migration creates:

- `search_projection_inbox`, deduplicated by source event ID;
- `search_projection_watermarks`, keyed by tenant, source module and projection scope.

Each inbox row stores the full envelope, scope, attempt count, retry time, bounded error and terminal state. `sys_events` and the outbox remain the platform event journal. Forum gains no Search table and `search_documents` is unchanged.

## Scope and watermarks

Topic/reply events, Forum module toggles, tenant/locale rebuild events and full Search/Forum reindex requests use the tenant-wide `forum` scope. A `forum_category` reindex keeps a category-specific watermark.

Category stale checks compare both the category watermark and the last completed full-scope watermark. Completing a category refresh does **not** advance the full-scope watermark. This prevents a targeted refresh for one category from incorrectly suppressing delayed work for a different category while still ensuring an old category refresh cannot run after a newer full rebuild or module disable.

## Claim serialization

All Forum projection operations for one tenant share a PostgreSQL advisory transaction lock. Search uses `pg_try_advisory_xact_lock`, not the blocking lock:

- one claimant keeps the transaction lock while its projector operation runs;
- competing dispatcher tasks immediately release their connection and leave their durable row pending;
- full and targeted Forum operations cannot execute concurrently;
- waiting claim tasks cannot exhaust the connection pool needed by the projector.

After taking the lock, Search selects the oldest non-terminal row with `FOR UPDATE`, ordered by `(revision_at, event_id)`. If that oldest row is in retry backoff, newer rows are not claimed. This is the strict retry barrier that prevents later work from overtaking a failed earlier projection.

## Projection and commit boundary

The existing Forum projector keeps its own Search transaction. The inbox transaction therefore does not atomically include document replacement, but it keeps the tenant advisory lock until completion:

1. enqueue the complete envelope;
2. acquire the non-blocking tenant Forum lock;
3. select the oldest row;
4. compare its effective watermark;
5. run the existing projector;
6. atomically commit the exact-scope watermark and terminal inbox state.

The watermark advances only after projection succeeds.

## Failure and replay

- Crash before claim leaves `pending` work.
- The `processing` update remains uncommitted while projection runs, so a crash rolls it back.
- Crash after projector commit but before inbox commit replays the idempotent Forum replacement.
- Projection errors use bounded exponential retry from 5 to 300 seconds.
- After 12 attempts, the row becomes terminal `dead_letter`.
- Duplicate event IDs do not create duplicate rows.
- Stale or equal revisions become terminal `skipped`.
- Malformed persisted envelopes become terminal `dead_letter`.

Forum events run a bounded reconciliation batch immediately. Other Search-relevant events opportunistically process a smaller batch. This slice does **not** add a startup worker or idle-tenant periodic sweep, so retryable work for an otherwise idle tenant remains a FORUM-20BQ concern.

## Compatibility

The slice adds one Search migration but no new runtime crate dependency, workspace dependency or `Cargo.lock` change. Fresh `rustok-search` dependencies, including `rustok-content`, remain intact; Tokio stays dev-only. No Forum REST route, GraphQL field, public DTO, Search query contract, canonical result URL or FFA/FBA readiness status changes.

PostgreSQL is required for runtime reconciliation. SQLite receives schema parity for migration/source-contract coverage only.

The large canonical Forum plan, Forum `CRATE_API.md` and Search local plan remain conflict-sensitive synchronization debt.

## Remaining work

FORUM-20BQ should add a host-owned startup or periodic due-inbox sweep, or continue FORUM-23 with safe author summaries, member projections and filter contracts. Owner-issued monotonic revisions remain necessary if cross-producer clock skew must be resolved without assumptions.

## Validation status

No tests, Cargo commands, formatting, verifiers, workflows or CI were run by the implementation agent.
