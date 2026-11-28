# Durable multi-replica platform build progress snapshots

- Date: 2026-11-28
- Decision status: Accepted
- Implementation status: In progress
- Owners: Platform Build / Server
- Extends: None
- Supersedes: None
- Superseded by: None

## Context

Platform build history and current status are persisted in the shared database,
but GraphQL `buildProgress` and `GET /ws/builds` consume a process-local
`BuildEventHub` broadcast. A build worker on another replica cannot publish
into that hub. A process restart or a lagged receiver can therefore lose the
only notification even though the canonical build row contains the newer
state. A new subscriber also has no atomic way to obtain the current build
snapshot and then follow later changes.

The publisher runs after persistence commits. Treating a notification failure
as a build execution failure is incorrect: the build state may already have
committed, and a transport failure must not rewrite that durable outcome.
Progress consumers need the latest committed state, not an exactly-once stream
of every intermediate progress sample.

## Decision

Use the shared `builds` table as the canonical source for a durable,
multi-replica *snapshot stream*.

- Add a singleton `build_event_clock` row and a non-negative
  `builds.event_sequence` watermark.
- Every build creation, claim, progress/status transition, and failure update
  reserves the next global sequence and writes it to the affected build row in
  the same database transaction.
- The clock row serializes sequence allocation until commit. A reader can
  therefore advance a global cursor without missing a later-committing
  transaction that received an earlier sequence.
- Same-status updates compare the observed `event_sequence` as well as the
  observed status. A stale progress writer cannot overwrite a newer snapshot.
- The owner-facing `BuildControl` port exposes the current sequence, active
  snapshots, one build snapshot, and changed snapshots after a cursor. Server
  transports continue to consume only framework-neutral API snapshots; they
  do not query SeaORM entities.
- `BuildEventHub` becomes a process-local wake-up channel only. Each GraphQL
  or WebSocket subscriber reads snapshots from `BuildControl` after its
  cursor, on local wake-up and on a bounded periodic poll. Other replicas are
  discovered through the shared database even when no local notification is
  received.
- A new subscriber reads the current sequence before subscribing, then
  subscribes before reading its initial active/build snapshot. It emits that
  snapshot first and filters duplicate sequence values while catching up.
- The public progress payload includes a decimal-string sequence and an
  `isSnapshot` marker. Existing event kinds and status fields remain
  compatible.
- If a build is updated several times between polls, the stream may coalesce
  those intermediate values and deliver the latest row snapshot. Build history
  remains the separate durable history query.

Build-state persistence is authoritative. Post-commit notification failure
must not convert a committed claim, progress update, completion, cancellation,
or failure into a different build outcome. Notifications are best-effort
wake-ups; periodic database polling is the recovery path. The separate
platform-composition outbox/retry contract for `BuildRequested` domain events
is unchanged.

Progress is a percentage in `0..=100`; it cannot decrease, and stages cannot
move backwards. Success always persists `Complete` and `100`.

## Sources of truth and ownership

- `rustok-build` owns build state transitions, progress sequencing, and
  progress queries.
- `rustok-migrations` composes the owner migration into the platform schema.
- `rustok-api` owns the framework-neutral `PlatformBuildProgressSnapshot`
  returned through the read-only `BuildControl` port.
- `apps/server` owns the local wake-up hub and GraphQL/WebSocket polling
  adapters.
- `builds.status`, `builds.stage`, `builds.progress`,
  `builds.error_message`, and `builds.event_sequence` are authoritative.
  The clock row allocates globally ordered progress watermarks. Local
  broadcast messages are disposable hints, never persisted authority.

## Invariants

1. Every committed build state change has a strictly increasing global
   `event_sequence` allocated in the same transaction as the row update.
2. A reader advancing its cursor never skips a later-committing build update
   with a lower sequence.
3. A stale same-status writer cannot overwrite a newer progress snapshot.
4. Progress remains within `0..=100`, does not decrease, and stages do not
   regress.
5. A subscriber can recover current active state after reconnect and observe
   subsequent changes committed by any replica sharing the database.
6. A local wake-up may be lost, duplicated, or delayed without losing the
   canonical build state or preventing later polling from observing it.
7. Notification failure after a committed build transition never changes the
   persisted build outcome.
8. Transports require the existing explicit host-global read authority and do
   not add tenant module-read requirements.

### Allowed states

- A new subscriber receives the latest active/build snapshot and then later
  snapshots with greater sequence values.
- A subscriber may receive the same sequence during initial snapshot catch-up;
  the transport deduplicates it.
- Several intermediate updates to one build may be coalesced into its latest
  snapshot.
- A replica that missed local notifications discovers committed updates by
  polling the shared database.

### Forbidden states

- A build update is visible only to subscribers attached to the writer's
  process.
- A transport advances its cursor using an uncommitted or non-transactional
  sequence allocation.
- A stale progress update overwrites a row with a newer sequence.
- A notification failure marks a successfully committed build transition as
  failed.
- A progress value is negative, greater than 100, or moves backwards.
- A stream silently treats a lagged local broadcast as proof that no update
  occurred.

## Non-goals

- This is a latest-state snapshot stream, not an immutable audit/event log; it
  does not promise delivery of every intermediate progress sample.
- It does not introduce a distributed build-worker lease or a global limit on
  concurrent build execution. Worker ownership/crash recovery remains a
  separate build-execution contract.
- It does not replace the transactional outbox for domain events or change the
  platform-composition idempotency receipt.
- It does not change host-token rotation semantics for already-upgraded
  WebSocket connections; the existing connection still requires reconnect
  after host-authority rotation.
- It does not change tenant/module authorization. Platform build visibility
  remains host-global and requires `HostAuthority::Read`.

## Data, transaction, and concurrency boundary

The singleton clock update and the build-row insert/update are performed on
the same `DatabaseTransaction`. Sequence allocation uses a guarded atomic
increment with `RETURNING`; missing, corrupt, unsupported, or exhausted clock
state fails closed. Existing builds start with sequence zero after migration.

A build update reads the current row, validates the status/stage/progress
transition, reserves the next sequence, and updates with predicates for the
observed status and sequence. A concurrent writer that loses the compare-and-
swap receives a conflict instead of overwriting a newer snapshot.

The read port queries rows with `event_sequence > cursor`, ordered by
sequence, and bounds each page. Because each build row stores its latest
sequence, multiple updates to one build between polls collapse to one latest
snapshot. The cursor is global across builds.

## Context dimensions

- Tenant: not an authorization dimension; platform build visibility is
  host-global.
- Principal/auth: every GraphQL and WebSocket entry still requires explicit
  host-global read authority; the WebSocket also retains its bearer-session
  lease checks.
- Trace, locale, channel, currency, timezone: not result-affecting inputs for
  the build progress snapshot.

## Events and projections

No durable event table or new outbox family is introduced. The build row is
the durable latest-state projection. The local hub only wakes subscribers;
polling the shared database is the reconciliation mechanism. Build history
continues to expose persisted build rows. The progress sequence is not a
replacement for the platform event bus or its outbox.

## Failure semantics

- Missing/corrupt/exhausted clock or database write failure: reject the build
  state transition; do not write an unsequenced snapshot.
- Local notification failure: retain the committed state and rely on polling.
- Poll/read failure: terminate the affected subscription or close the
  WebSocket with an internal-error close; clients may reconnect and obtain a
  fresh snapshot.
- Lagged local receiver: record telemetry and immediately query the durable
  cursor; never infer that no update occurred.
- Concurrent stale progress write: reject it without overwriting the newer
  state.
- Invalid progress/stage transition: reject before committing.

## Migration and cutover

Add `builds.event_sequence BIGINT NOT NULL DEFAULT 0`, an index on that
column, and the singleton `build_event_clock` row initialized to sequence
zero. Existing build rows retain their current state and begin at sequence
zero; no history is fabricated. The migration is additive and can be reversed
without deleting build history.

All server replicas must run the migration before enabling the database-backed
stream. A mixed deployment with old writers that do not update
`event_sequence` is not supported; roll out the schema and application
together.

## Alternatives considered

- **Process-local broadcast only:** rejected because it cannot observe updates
  committed by another replica and cannot recover after restart.
- **Redis Pub/Sub only:** rejected because messages are not durable and a
  disconnected replica still misses updates.
- **Full immutable build-event table:** rejected for this UI contract because
  consumers need the latest state, not every progress tick; a per-build
  watermark plus shared cursor avoids an unbounded event log and retention
  worker.
- **Poll `updated_at` only:** rejected because timestamp precision, clock
  behavior, and concurrent commits do not provide a reliable global cursor.
- **Treat notification failure as execution failure:** rejected because the
  database transition may already be committed and transport failure cannot
  reverse it.

## Verification

Regression coverage must prove atomic sequence allocation for build creation,
claim, progress, completion, and failure; stale same-status CAS rejection;
progress bounds/monotonicity; initial snapshots and cursor deduplication;
cross-hub visibility through one shared database; recovery after a missed
local wake-up; and notification failures that do not rewrite committed state.

Source guards and authored tests are not runtime certification. Rust
compilation, focused crate tests, PostgreSQL concurrency tests, GraphQL
subscription tests, WebSocket tests, and multi-replica fault injection remain
required before deployment.

## Consequences

The singleton clock row serializes build state writes, but build progress
writes are low-volume and the ordering guarantee is valuable. Subscribers use
a bounded database poll (accelerated by local wake-ups), adding small read load
in exchange for replica-independent delivery and restart recovery. The stream
is explicitly a latest-state feed; consumers that need a complete audit trail
must use a separate durable event contract.
