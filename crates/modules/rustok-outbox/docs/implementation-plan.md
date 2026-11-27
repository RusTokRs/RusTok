# Implementation plan for `rustok-outbox`

## Current state

`rustok-outbox` owns transactional event publishing, relay, retry, DLQ
semantics, and relay-worker control. The server and other modules consume this
runtime through owner contracts; they must not reimplement event delivery or
relay lifecycle.

`TransactionalEventWriter` is the object-safe write-side port for services that
already own a SeaORM transaction. `OutboxTransport` is its platform adapter;
domain operations receive the port through composition and cannot silently fall
back to non-transactional publication.

The read-only admin surface uses a module-owned core, transport facade, and UI
adapter. Native transport uses `HostRuntimeContext`. `OutboxRelayPort` uses the
canonical `rustok_api::ports` write policy, including deadline and idempotency
semantics; the owner crate exposes the required adapter feature.

The read-only operator dashboard is an accepted single-adapter owner fragment:
it has no public/headless outbox-admin contract, so its native `#[server]`
bootstrap remains the only package transport and no GraphQL fallback is added.

Tenant-facing DLQ administration is fail-closed. REST list and native counters
derive tenant scope from trusted request context, while replay requires
`logs:manage` and looks up the event by both id and tenant. Both REST DLQ
selection/replay and native counters prefer the canonical top-level envelope
tenant and consult the nested legacy tenant only when the canonical key is
absent. A conflicting nested tenant never grants visibility. Tenant admins
cannot select another tenant or inspect platform-global relay state through
these surfaces.

## FFA/FBA status block

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `OutboxRelayPort` / `outbox.relay_control.v1` in
  `crates/modules/rustok-outbox/contracts/outbox-fba-registry.json`.
- Static and runtime-order evidence:
  `crates/modules/rustok-outbox/contracts/evidence/outbox-contract-test-static-matrix.json`
  and `crates/modules/rustok-outbox/contracts/evidence/outbox-provider-runtime-order-smoke.json`.
- `npm run verify:outbox:admin-boundary` and `npm run verify:outbox:fba` lock
  the UI boundary, provider metadata, owner-service invocation order, and
  tenant/RBAC invariants for DLQ administration.
- The server relay worker consumes `OutboxRelayPort::process_pending_once` with
  a service actor, deadline, and per-tick idempotency key; it does not invoke
  the relay service method directly.
- Transactional outbox integration tests live in this owner crate; the
  foundational `rustok-core` crate remains independent from outbox runtime
  implementations, including in its development dependency graph.
- A representative sealed Blog Comments schedule-audit PostgreSQL packet is
  retained in
  `crates/modules/rustok-outbox/contracts/evidence/blog-comments-audit-relay-postgres-source.json`.
  Its outbox-owned harness covers retry, relay-owner reconstruction, delivery
  acknowledgement ordering and attempt-budget DLQ transition for the exact
  write-once envelope identity. The packet is source-ready and unexecuted; it
  does not promote FFA/FBA or close the broader durable consumer-completion gap.

## Open results

1. **Close the transport-to-consumer durability gap.** `sys_events=dispatched`
   currently means the configured `EventTransport::publish` returned success.
   In local fan-out modes, module handlers can still fail after that point or
   miss events through broadcast lag without a durable consumer receipt,
   consumer DLQ, or automatic source rebuild.
   **Depends on:** server event transport, `rustok-core` dispatcher semantics,
   and projection consumers such as Search.
   **Done when:** terminal consumer failure is durably observable and replayable,
   restart/lag recovery is executable, and projection consumers prove
   idempotent duplicate/out-of-order handling.

2. **Execute relay, backlog, retry, and DLQ runtime contracts.** Replace static
   evidence with targeted provider execution and fallback proof for relay
   control before any FBA promotion. The Blog Comments schedule-audit relay
   packet now supplies representative PostgreSQL harness source, but its
   maintainer execution remains pending and is not accepted runtime evidence.
   **Depends on:** a runtime-composed relay and representative delivery
   failures.
   **Done when:** transactional publish, retry, DLQ transition, degraded mode,
   and typed port errors are covered by executable tests.

3. **Prepare safe incremental operational adoption.** Define rollout,
   migration, tenant, RBAC, and security requirements that belong to relay
   control rather than to the host UI.
   **Depends on:** deployment topology and operator authorization model.
   **Done when:** staged enablement has explicit guardrails, permissions, and a
   rollback path without duplicated relay ownership.

4. **Maintain observability and incident guidance with delivery semantics.**
   Update metrics, alerting, and the runbook whenever relay/backlog/DLQ behavior
   changes.
   **Depends on:** the changed outbox runtime contract.
   **Done when:** operators can identify stalled relay, growing backlog, DLQ,
   consumer-side terminal failures, and retry failures with an owner-specific
   recovery procedure.

## Verification

- `npm run verify:outbox:admin-boundary`
- `npm run test:verify:outbox:admin-boundary`
- `node scripts/verify/verify-outbox-dlq-tenant-rbac.mjs`
- `node scripts/verify/verify-blog-comments-audit-outbox-relay-postgres-source.mjs`
- `npm run verify:outbox:fba`
- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- Targeted transactional publish, relay, retry, and DLQ runtime tests cover
  tenant isolation and consumer recovery.

## Change rules

1. Keep transactional publishing and relay policy in this module.
2. Update root/local docs and `rustok-module.toml` with a public event-runtime
   contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `blocked`
- Last verified at (UTC): `2026-07-30`
- Scope inspected: `relay claim/dispatch/retry/DLQ state machine, REST DLQ list/replay, native admin counters, tenant/RBAC trust boundaries, and server local-delivery composition`
- Findings: `P0=1, P1=1, P2=0, P3=0`
- Fixed in this pass: `P0 tenant isolation and authorization: tenant admins can no longer select or inspect another tenant's DLQ events; replay now requires logs:manage and a tenant-qualified event lookup; native operational counters require tenant context and are tenant-scoped`
- Remaining risks or blockers: `P1: outbox dispatched state proves transport acceptance only; local module handler terminal failure or broadcast lag can leave Search and other projections stale without a durable consumer receipt/DLQ/replay contract`
- Evidence: `source regressions in admin_events.rs and rustok-outbox-admin native adapter; scripts/verify/verify-outbox-dlq-tenant-rbac.mjs is included by npm run verify:outbox:fba; same-SHA Actions are required before merge`
- Next action: `design and execute a durable consumer completion/recovery contract with server event delivery, rustok-core dispatcher, and Search projection owners; then run relay and restart E2E evidence`
- Resume command: `node scripts/verify/verify-outbox-dlq-tenant-rbac.mjs && npm run verify:outbox:fba && cargo test -p rustok-outbox && cargo test -p rustok-server admin_events`

## Unconditional publication row admission (2026-11-01; source only)

Unconditional root and sealed-contract publication now require exactly one affected insert
row. A suppressed/zero-row insert returns persistence failure instead of acknowledging a
mutation without its required event. The existing separate write-once `ON CONFLICT` path
still reads/correlates an exact durable envelope; zero insertion alone never proves replay.
This strengthens the canonical transport, not an Inventory-specific pipeline or schema.

Two additional pure Outbox definitions and the native I/C suppressed/missing-outbox rollback
fixtures are source only and unrun. See [Inventory source evidence](../../rustok-inventory/contracts/evidence/native-stock-outbox-source.json).
No Rust/SQL/relay or consumer durability certification follows. The transport-to-consumer
completion gap above remains open; local state/outbox atomicity is not delivery atomicity.

## Reservation sealed-fact continuation (2026-11-01; source only)

A fresh canonical transaction-only helper now lets Inventory append its sealed reservation fact
on its counter/receipt transaction. Existing relay dispatch/retry/DLQ owns delivery; no migration
or alternate queue. Server targets await explicit local sealed handlers; Iggy-first/local-ack
retry preserves the original identity and is at-least-once. Compilation, actual SQL rollback,
configured relay/Iggy receive/cursor ACK, crash/restart and DLQ evidence remain open. See the
[API](../CRATE_API.md#fresh-sealed-facts-and-local-acknowledgement-2026-11-01-source-only).

## Dispatch admission and envelope identity (2026-11-27, source only)

Refresh the exact pending claim at dispatch admission and check the remaining
lease budget before publication. Root and sealed-contract relay and storage
replay reject conflicting durable identity and populated tenant projections.
Historical null projections remain readable. Source regressions cover queued
renewal, stale claims and both envelope families. Compilation, database
contention, real target cancellation and broker idempotency remain unverified.

## Retention consumer fencing (2026-11-27, source only)

Both delivered and opt-in DLQ event deletion use the same consumer hold in
candidate selection and final DELETE. PostgreSQL serializes registry writes
with the deletion transaction using a SHARE table lock before the fresh
READ COMMITTED snapshot; SQLite serializes writers. The SQLite DLQ hold
regression was extended. PostgreSQL registration/delete contention and
runtime verification remain pending.

## Durable consumer lifecycle and acknowledgement authority (2026-11-27, source only)

Consumer registration and retirement serialize registry mutations before drain
reads. PostgreSQL retirement takes a non-blocking event-table writer fence;
SQLite reserves the writer before scanning. Drain requires matching event-type
receipts. ACK holds registry and source event authority through the owner
transaction; contention must roll back owner effects and be retried.
Unfenced transport ACK cannot complete a relay claim.
Source regressions cover retirement, wrong-type receipts and claim stealing.
PostgreSQL contention, owner rollback/retry, compilation and broker delivery
remain unverified.

## Database-authoritative lease clock (2026-11-27, source only)

Durable relay claims, retry deadlines, storage timestamps and retention cutoffs
use SQL database time. PostgreSQL uses clock_timestamp(), not transaction-start
time; SQLite uses its SQL clock. Receipt renewals and reclaims use a PostgreSQL
write-side clock expression; reclaim compares the previous token and timestamp.
Relay renewal rotates its token even within the same clock tick and counts
database wait time in its monotonic dispatch budget. Clock errors fail closed.
Relay and retention intervals are bounded to 3650 days. This does not fence
primary clock discontinuities or guarantee exactly-once broker effects.
Source regressions cover clock progress, token rotation and capped backoff.
PostgreSQL contention, primary clock discontinuity handling, retry jitter and
per-pass metrics remain pending. No compilation or tests were run.

## Relay execution and retry hardening (2026-11-27, source only)

Relay passes reserve at most max_concurrency rows across their sliding window and
advance a stable (created_at, id) cursor within the invocation. Due/reclaim
eligibility uses one initial SQL-clock horizon. A shared pass guard serializes
clones of a relay instance, including aborted tasks until they actually exit.
Independent updated relay instances/processes share a durable schema-local
execution quota via SQL claims. Reports count the joined
durable outcomes of the specific pass, not differences of lifetime counters.
A failed or cancelled pass can have committed earlier outcomes.
Retry uses equal jitter between half the capped exponential delay and the cap
(with a one millisecond floor). Startup validates relay configuration before
connecting broker transports. The sliding window replenishes available slots without waiting for a full wave.
Regression source covers gated slot admission, concurrent clone calls,
per-pass accounting and jitter endpoints. Compilation, cancellation contention,
PostgreSQL multi-process dispatch and broker behavior remain unverified.

## Strict owner-operation lease expiry

Owner-operation receipts now enforce a strict five-minute half-open live
interval. A matching token cannot renew, lock, finalize or release an expired
receipt. Sample SQL time after acquiring the receipt write lock, recheck
expiry on reclaim and restamp fresh admission after unique-index contention.
Completion and failure use a transaction or an owner-transaction savepoint;
owner errors require rollback of the entire owner transaction. Expired release
preserves the request fingerprint for exact-request reclamation. Owners must
renew before expiry and keep owner transactions bounded; a row lock alone
does not extend the lease. External effects need owner-specific idempotency.

Source regressions cover expiry, future timestamps, stale operations and reclaimed completion. Generic finalization/release now requires TransactionTrait; cross-owner compilation and PostgreSQL lock-wait verification remain open.

## Durable temporal clock authority

The outbox_temporal_clock singleton stores the maximum committed SQL-clock
observation across process restarts. Independent relay-pass, receipt admission,
renewal/reclaim and retention boundaries advance it in a short transaction.
Owner transactions read the floor and SQL time without taking the global writer
lock. Missing schema, backward observations below the committed floor, slow
observations (>2s), host-clock steps (>100ms during observation) or SQL/host
skew outside a 2s sanity bound fail closed; host time is not a timestamp
fallback. Relay claim refresh checks its live interval after acquiring row
ownership. Event retention checks time again after the registry fence.

This is bounded discontinuity detection, not a trusted independent distributed
time service: simultaneous host/DB forward jumps, smaller jumps within the
sanity bound and a stale repeatable-read snapshot are not fully fenced.
A floor ahead of actual time blocks affected operations until time catches up
or an explicitly reviewed operator recovery is performed. Do not reset it on
startup. The floor begins at zero on migration; pre-upgrade history is not
retroactively verified. Upgrade the schema before new workers and drain old
workers that bypass the guard. A committed owner transaction may still be
subject to cancellation and ambiguous broker effects.

Migration composition and source regressions are unverified at runtime. Deployment-wide clock latching and broker ACK remain open.

## Distributed execution admission (2026-11-27)

The outbox_execution_policy singleton owns a per-database-schema budget
(max_in_flight, default 8, range 1..1024) and the shared claim_ttl_ms.
The first updated relay binds the TTL; subsequent embedded and standalone
relays must match it before starting their brokers or claiming work.
The local max_concurrency remains an upper bound, independent of the cluster
budget. Existing sys_events claimed_by/claimed_at pairs are the only durable
execution permits. Claim and refresh lock the policy before event rows;
PostgreSQL uses READ COMMITTED, SKIP LOCKED for candidates and NOWAIT for
refresh. The policy lock is released before any broker effect. Completed
claims release capacity through existing conditional state transitions.
Cancelled or crashed publications may retain reservations until expiry;
transport effects remain at-least-once and may outlive cancellation.
A sliding window replenishes a slot after a joined outcome rather than waiting
for a whole wave. Claim failures stop new admission and drain active tasks.
The pass cursor and initial eligibility horizon are preserved.

Deployment: stop and drain all old relay processes before applying
m20261127_000201_create_outbox_execution_policy. Old binaries bypass the
budget. Ensure no old in-flight publications remain before starting new
relays. Apply the migration before starting new workers; the schema helper
does not reset an existing policy. For a deliberate policy change, stop and
drain every relay, verify there are no live or ambiguous claims, then change
the singleton max_in_flight and claim_ttl_ms through controlled database
maintenance and align all runtime TTL settings before restart. No live
policy editing API is provided; online cap reduction or TTL change is not
supported. Multiple databases/schemas have independent budgets. Direct SQL
claim mutations outside the protocol, primary clock anomalies, uncooperative
transport cancellation and broker-side fencing remain outside this guarantee.

Source regression: independent relay instances sharing a two-slot budget, incompatible TTL rejection and sliding-window replenishment. PostgreSQL multi-connection contention, cancellation and migration verification remain open. No build or tests run.

## Publication identity and acknowledgement boundary (2026-11-27)

Canonical root and sealed event UUIDs now populate the Iggy message header
ID without translation. Non-UUID publication keys, including the existing
DLQ replay attempt keys, map to a fixed namespaced UUIDv5; nil UUIDs and
empty keys are rejected. The namespace and the existing partition routing
must remain stable across retries. Explicit direct-mode publication awaits
the broker reply rather than accepting background enqueue as delivery.
No-SDK external connections and publications fail closed. The dedicated
poison-DLQ path retains its existing identity and partition mapping.
The strength of the broker response still depends on topic/server durability
and the reviewed broker deduplication policy. Successful direct send is not
a physical persistence proof or consumer cursor acknowledgement. An empty
confirmation list can be legitimate (including a duplicate response); no
broker offsets or persistence guarantees are inferred. Older publications
without the header identity cannot be retroactively deduplicated. Changing
topology, partition count or broker destination requires coordinated drain
and recovery. Do not treat SDK producer idempotence as cross-process Outbox
exactly-once delivery.

The relay sends invalid stored envelopes directly to its DLQ before invoking
the target. Every target error, including validation/serialization after a
remote send or a local projection, consumes the bounded ordinary retry
budget instead of assuming there was no side effect. Timeout outcomes remain
unconfirmed and may redeliver the same identity. A successful transport
reply followed by SQL failure or process death may also redeliver; the
existing fenced SQL acknowledgement and consumer idempotency still apply.
No transaction spans the database and the broker.

Source regressions cover stable UUID/replay IDs, SDK message header, fail-closed no-SDK mode, and a post-acceptance validation failure followed by successful retry. Compilation, actual broker response and restart/deduplication-window calibration remain unverified. Existing retained runtime evidence was not regenerated.
