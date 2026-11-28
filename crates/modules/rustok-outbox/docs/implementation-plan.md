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

## Consumer ACK admission and retention bounds (source only)

PostgreSQL consumer ACK reserves the source event row exclusively with NOWAIT
and reads its subscription with NOWAIT under the registry SHARE fence. This
serializes canonical ACK writers for one event without waiting while holding
owner locks. Contention requires rollback and retry of the owner transaction.
Trusted backfill may skip a missing event only for a currently registered
consumer. Live ACK still requires an existing event and exact event type.
Event and terminal-receipt pruning accept batches of 1..=10000; direct receipt
pruning also enforces the positive, at-most-3650-day window before SQL.
Source regressions are unrun; PostgreSQL multi-connection contention,
compilation and runtime verification remain open.

## Durable execution policy integrity (source only)

The append-only migration `m20261127_000202_fence_outbox_execution_policy`
and standalone owner schema install the same database guards. The singleton
cannot be inserted, deleted, replaced, truncated (PostgreSQL), change identity
or reset a bound TTL to NULL through ordinary SQL. Exact no-op reservation
updates remain valid. First TTL binding and later cap/TTL changes require
**all** claims to be absent, including expired or ambiguous claims.
PostgreSQL event claim writes take the existing policy row NOWAIT and write
it before checking live capacity; canonical relay admissions already hold
that row. The write fences stale repeatable-read admissions. SQLite serializes
writers and validates integer policy values and claim timestamp shape. Both
backends reject unbound policies, malformed claim pairs and acquisitions that
would exceed the live claim cap. Expired claims may exist for normal reclaim.
There is no second reservation table.

Stop and drain all relays, reconcile ambiguous external publications and
clear any remaining claims through reviewed recovery before this migration or
a policy change. The migration rejects missing/corrupt singleton state and
any outstanding claims; it never silently resets a policy. Apply it before
restarting workers. Offline downgrade also requires drained workers.
The guards enforce ordinary DML quota and policy integrity, not database
superuser/DDL authority, the identity of an arbitrary SQL writer that can
clear or replace a token, clock discontinuities, or fencing at a remote broker.
The PostgreSQL per-claim policy write and SQLite timestamp conversion in the
quota guard need workload measurement. Compilation, SQL migration lifecycle,
multi-connection contention and broker behavior remain unverified.

## Durable temporal authority integrity (source only)

Append-only migration `m20261127_000203_fence_outbox_temporal_clock`
and standalone schema guard the permanent clock singleton. Ordinary INSERT,
DELETE, replacement, PostgreSQL TRUNCATE, identity changes, fractional
SQLite values, decreasing floors and advances past SQL time are rejected.
Exact no-op reservations remain valid. Existing missing singletons are
not reseeded; migration validates the baseline under a writer fence.
A preexisting future floor is preserved and observations fail closed.

The guards protect the durable floor, not independent distributed time,
privileged DDL/restore or simultaneous primary/host clock jumps. Owner
transactions still do not take a shared clock writer lock. Drain old
binaries and use reviewed offline recovery; never reset at startup.
Both integrity installers now open a transaction/savepoint even for
direct standalone setup. PostgreSQL duplicate DDL errors are handled
within SQL, not by continuing an aborted transaction. SQLite and
PostgreSQL regression sources are authored; compilation, migration
lifecycle and contention evidence remain pending.

## Crash-safe dispatch attempt ledger (source only)

Before invoking a transport, the relay persists an `outbox_dispatch_attempts`
intent in a short transaction under the exact pending event claim. The attempt
UUID is local diagnostic identity; the original event UUID remains the stable
broker publication identity. At most one open attempt exists per event.
The ledger records `open`, `confirmed`, `uncertain` and `not_started`.
A transport error or timeout is uncertain, not proof of non-delivery.
Completion and event ACK/retry/DLQ are atomic. A lost claim cannot finalize
its successor. If the process dies after the intent commit, the next relay
claim closes that open attempt as uncertain, charges one retry failure and
schedules ordinary backoff or DLQ before another publish. Recovery itself is
not another external effect. A proven pre-call lease-budget failure closes the
intent as not_started and releases the claim without charging retry budget.
Invalid envelopes still go directly to DLQ without a transport intent.

Migration `m20261127_000204_create_outbox_dispatch_attempts` adds the owner
schema with event FK cascade and an indexed single-open-attempt invariant.
Retained event history includes its attempts; event retention cascades them.
DLQ replay resets the event retry budget, not the historical attempt rows.
This is conservative at-least-once: a crash after intent but before invocation
is indistinguishable from a crash after broker acceptance. The journal does
not establish exactly-once, broker acknowledgement durability, per-consumer
success, or protection against arbitrary privileged DML. Stop/drain old relays
before migrating and do not mix old writers with ledger-aware relays.
Regression sources are authored; compilation, SQL lifecycle and crash/remote
effect evidence remain pending.

An unclaimed transport ACK and both retention paths now also reject events with
an open dispatch intent, even if an external SQL mutation cleared the claim or
changed status. Closed attempt history still follows event retention. A known
pre-intent budget exhaustion releases its exact claim without a retry charge.
PostgreSQL/SQLite integration regression sources check outcome history,
rollback on a lost journal row, and retention of unresolved evidence; unrun.

## Transaction-bound consumer effect admission (source only)

Notifications intake serializes terminal acceptance and rejection on the
canonical source row before reading either owner outcome. PostgreSQL uses
READ COMMITTED and FOR UPDATE NOWAIT; SQLite reserves its writer before
reading. Decode uses the locked source snapshot. Acceptance writes run in
a savepoint, rolled back before a permanent rejection is persisted; retryable
errors abort the whole transaction. An existing terminal outcome is replayed
without another owner write. A legacy contradictory accepted/rejected pair
fails closed and requires reviewed reconciliation. The terminal owner
outcome, source inbox and generic retention ACK commit together. No broker
or external I/O is allowed in this transaction.

OutboxConsumer::reserve_effect_in_transaction exposes the existing exact
consumer/event receipt insertion result as a transaction-local admission:
true means a new receipt, false means an exact prior committed receipt
(or one inserted earlier in the same transaction). Perform the SQL owner
effect only on true and commit receipt and effect together; roll back on
any error. It is not a reservation that can be committed separately and
does not cover external effects, broker replay after event retention, or
unregistered consumers. Existing late ACK/backfill semantics remain intact.
This does not automatically make arbitrary module handlers idempotent.
SQLite and PostgreSQL regression sources are authored, not executed.

## Durable fanout recovery (source only)

The relay pins a versioned delivery route per event before invoking any target
or wrapper. An incompatible route refuses delivery before external I/O; review
and drain pending/DLQ events before changing profiles or broker routing.
The server external-Iggy fanout records primary acceptance in a separate
short SQL transaction under the exact live claim and open dispatch intent.
On retry, a committed checkpoint skips the primary call and retries local
delivery. A crash between broker acceptance and checkpoint commit is still
ambiguous and can redeliver. The original event UUID remains the broker ID.
The same route also pins whether local delivery was required when fanout
first reached its validated registry. A mismatch fails closed.

Local handlers, cache rotations and other process-local effects are not
skipped on the basis of another host's success. Bundled Iggy is deliberately
not eligible for primary skipping because the configured local path is not
proof of shared durable broker identity. External configuration fingerprint
covers credential-free endpoint routing, protocol/TLS domain, stream and
serialization, not remote broker identity or retention. Replacing a broker
behind the same endpoints requires reviewed recovery; never reuse a route
checkpoint against a different broker. The standalone broker-only relay has
a distinct route from the server fanout; do not run different route owners
against the same pending queue. Generic library targets use an unclassified
route unless the host explicitly supplies a stable one.

The route and checkpoint share the event retention lifecycle, including DLQ
replay; replay does not silently discard confirmed evidence. Old relay
binaries must be stopped/drained before migration
`m20261127_000205_create_outbox_delivery_checkpoints`. No SQL transaction
spans broker I/O. This is not an exactly-once guarantee, a complete
per-handler receipt system, or a proof that the broker retained an accepted
message. Source regression tests are authored, not executed.

A changed bound route is now classified as a pre-publication validation
failure through the normal bounded retry/backoff and DLQ policy, releasing
the exact claim rather than monopolizing capacity until lease expiry.
Checkpoint-enabled primary publication refuses without its relay task scope;
the test-only uncheckpointed transport retains ordinary direct behavior.
The server regression exercises local failure, relay reconstruction and
confirmed-primary skipping with a transport double (not a live broker).

Relay route configuration is validated before acquiring any event claim or
entering the polling loop. A wrapper cannot disable checkpointing after a
primary confirmation has been persisted for that event; the relay rejects
that invocation before primary I/O, preserving the existing checkpoint.
Focused source regressions cover both paths; execution remains unverified.

Terminal owner-operation receipt finalization rejects retryable `PortError`
values before touching storage. Owners must use `release_retryable` after
rollback for these errors; accidental terminal finalization can no longer
poison subsequent replay with a temporary outage. The focused receipt
regression is source-only and has not been executed.

## Exact durable dispatch admission (2026-11-27; source only)

Claim batches must write every selected locked candidate and read back exactly
that many claimed rows before commit. A suppressed write aborts the whole
batch. Claim renewal returns the actual row read under the new claim instead
of patching a queued snapshot in memory. Before any external publication,
route checkpoint and open attempt creation must each insert exactly one row
and match the readback identity and initial state in the same transaction.
Suppressed or altered inserts abort admission and roll back any newly created
checkpoint/intent; the pending claim remains reclaimable under its lease.
No retry is charged for this storage-admission error. SQLite source regressions
cover suppressed claim, checkpoint and attempt writes and fresh retry metadata.
This does not make arbitrary later SQL mutation safe or guarantee exactly-once.
Compilation and regression execution remain pending.

## Database-enforced delivery history (2026-11-28; source only)

Migration `m20261128_000206_fence_outbox_delivery_history` and standalone schema setup use the same
transactional installer. Attempt identity is immutable; inserts require a pending
matching claim and route, and an open attempt may close only once with an event
state consistent with its outcome. The event transition and closure still commit
in one owner transaction. A route starts unconfirmed, binds topology once and
confirms primary once under a matching open claim. PostgreSQL child writers
reserve the parent NOWAIT rather than wait in reverse lock order. Direct child
deletion while its event exists, closed-attempt reopening, checkpoint reset,
SQLite child REPLACE (including rowid collisions), and PostgreSQL TRUNCATE are
rejected. Open attempts also prevent direct parent deletion; ordinary terminal
event retention still cascades closed history. Existing history is preserved
without inventing checkpoint evidence for pre-checkpoint attempts.
These guards enforce structural transitions, not the truth of a broker ACK:
SQL with sufficient privileges can still manufacture a valid-looking transition.
Parent identity/SQLite parent REPLACE, arbitrary privileged DDL, restored backups,
and proof of retention eligibility against direct parent DELETE are separate
boundaries. Stop/drain older relay binaries before deployment. SQLite and
PostgreSQL regression sources are authored, not executed.

The idempotent transport ACK fallback now requires an unclaimed dispatched
row without any open attempt in one read snapshot. A contradictory external
status-only update cannot report successful ACK or permit retention while
unresolved evidence exists. A SQLite source regression covers this anomaly
and explicit closure/retention; execution remains pending.

## Exact dispatch outcome integrity (source only)

Relay writer fences verify the complete event snapshot. ACK, retry, DLQ and
pre-publication release read back the expected event transition before commit;
closing an attempt requires its exact claim token and verifies its terminal
record. Recovery explicitly uses the abandoned intent's token, not the new
claim's token. Fanout topology and primary confirmation also require exact
checkpoint readback. Rewritten outcomes abort the SQL transaction instead of
reporting success or silently losing retry state. No broker I/O is added to
these transactions; ambiguous broker effects still require recovery.
SQLite regression sources exercise rewritten event and attempt ACK outcomes
independently of the history guard; compilation and execution remain pending.

## Durable retryable receipt release (source only)

A retryable release changes `processing` to `released` under the exact live
lease, rotates its token and retains the original operation UUID and request
fingerprint. Only the identical request can immediately re-admit a released
receipt; a different operation or payload conflicts. Re-admission locks the
prior state and token, compares the stored snapshot, rotates the token again
and verifies the resulting row before commit. Release verifies its readback
as well. Released receipts are not terminal and are excluded from retention;
abandoned keys therefore remain reserved until the exact command completes
or an explicitly reviewed recovery is performed. This changes no schema
columns; the status is stored in the existing string column. Stop/drain old
receipt writers before rollout: older versions delete on release and do not
recognize `released`. Previously deleted fingerprints cannot be reconstructed.
SQLite source regression is extended; compilation and PostgreSQL concurrent
admission evidence remain pending.

## Immutable source event envelope (2026-11-28; source only)

Migration `m20261128_000207_fence_outbox_event_envelope` shares its installer with standalone schema setup.
The original event id, tenant projection, event type, schema version, payload
and creation timestamp cannot change through ordinary UPDATE. Retry, ACK and
DLQ replay retain that identity. SQLite forbids rowid mutation and different
physical UUID aliases; an exact-key duplicate INSERT (including REPLACE) is
suppressed without changing the original row or cascading its history. This
preserves the existing write-once readback/conflict protocol and unconditional
inserts still require one affected row. The unique normalized UUID index rejects
preexisting alias collisions at migration rather than discarding history.
PostgreSQL legacy tenant backfill considers nested tenant only if the top-level
key is absent, leaves nil tenant unprojected and does not rerun after the guard.
This is not retrospective repair of previously incorrect projections.

Parent deletion now requires a terminal, unclaimed event and all registered
consumer ACKs. PostgreSQL requires READ COMMITTED and takes the registry SHARE
fence NOWAIT in a direct delete, avoiding a blocking reverse lock order;
SQLite serializes writes. Existing open-attempt protection remains in force.
Retention queries exclude claimed rows. Configured retention age and failed
retention enablement remain enforced by the application, not by these SQL guards;
a privileged direct terminal DELETE can still bypass the age policy. Event IDs
can be reused after lawful pruning; permanent deduplication/tombstones are not
introduced. No claim of protection from arbitrary DDL or restored backups.
Drain writers before migration. PostgreSQL/SQLite regressions are authored;
compilation, migration and contention execution remain pending.

## Durable event retention policy (2026-11-28; source only)

Migration `m20261128_000208_create_outbox_event_retention_policy` and standalone schema share a singleton
`outbox_event_retention_policy`. A database starts unbound: event deletion
fails closed until the first valid pruner binds the delivered and optional DLQ
windows. A NULL failed window means keep DLQ indefinitely. Every later pruner
must match both windows (whole milliseconds, 1..=3650 days), including on an
empty queue. Local batch size and cadence remain independent. Before every
bounded event delete, the pruner rechecks the durable values under the policy
writer lock, then takes the consumer registry fence and event locks. The SQL
DELETE trigger independently enforces the enabled window, event age and SQL
clock floor. PostgreSQL direct DELETE takes the policy row SHARE NOWAIT and
requires READ COMMITTED; SQLite uses its serialized writer. Previous consumer,
claim and open-attempt holds remain mandatory. The singleton cannot be deleted,
replaced, truncated or reset to unbound, and schema setup never reseeds it.

For coordinated reconfiguration, stop/drain all retention workers, review the
new deletion horizon and update both windows in a controlled SQL transaction
on the existing singleton, align runtime settings and restart. An old worker
with mismatched settings fails closed rather than overwriting policy. Direct
SQL with policy-write authority can still change windows; this is not a
privilege boundary. Stop old binaries before migration. Owner-operation
receipt retention remains a separate, not yet durably bound contract. Event
status and dispatched_at mutation, privileged DDL/restore, and independent
clock discontinuities are separate remaining boundaries. SQLite/PostgreSQL
regression sources added; compilation and runtime evidence remain pending.

## Exact initial event persistence (source only)

Fresh root and sealed-contract inserts require one affected row and exact
readback of the complete initial event model, including pending status, zero
retry count, absent claim and canonical payload. Write-once contract inserts
check the fresh-write path separately from an existing identity replay.
Standalone root publication holds one SQL transaction through insert/readback
and commit. Owner transaction append errors require rollback of the owner
transaction; no external I/O occurs in these checks. A rewritten insert cannot
be acknowledged as a successful fresh event. Existing duplicate replay still
compares canonical facts rather than regenerated timestamps or trace metadata.
SQLite regression source covers rewritten standalone and owner-transaction
inserts; compilation and execution remain pending.
## Database-enforced event state transitions (2026-11-28; source only)

Forward migration `m20261128_000209_fence_outbox_event_state` installs the
same event-state guard in PostgreSQL and SQLite. New rows must enter as pristine
`pending` events. While pending, only a monotonic claim acquire/rotate/release,
a retry charged against one open dispatch intent, a terminal validation or
dispatch failure, or a valid delivery transition is accepted. Delivery requires
the exact current open attempt, or the legacy unfenced ACK path on an unclaimed
row with no open attempt. `dispatched_at` cannot be rewritten after delivery.
The only terminal-to-pending edge is DLQ replay with closed attempt history,
zero retry count, cleared retry metadata and no live claim.

Retry deadlines are bounded relative to the claim time; the retry counter fails
closed at its integer limit. The trigger reads attempt history without taking
child-row locks, preserving parent-event-before-attempt lock order. Claim
timestamps cannot be backdated or renewed with the same token. ACK and admin
DLQ replay now retain their SQL transaction through exact readback; concurrent
replay has a single conditional-update winner.

The guard enforces the legal state graph, not caller identity or broker truth:
a database principal able to issue a structurally valid transition can still
forge a plausible outcome. Arbitrary privileged DDL, restored backups, and
external broker ambiguity remain separate trust boundaries. Drain old writers
before migration. PostgreSQL/SQLite regression sources were updated or added;
compilation and execution remain pending.

## Exact owner-operation receipt transitions (source only)

Fresh admission verifies the inserted receipt before and after restamping its
lease following possible unique-index contention. Renewal and terminal success
or failure compare the full persisted model to the expected transition before
commit, including request identity, token, timestamps and result/error payload.
Terminal readback mismatch explicitly rolls back the nested receipt savepoint;
callers must still roll back the entire business transaction on receipt error.
PostgreSQL acquires the exact receipt row with FOR UPDATE NOWAIT; SQLite reserves
the writer with a zero-row write. Neither lock path fires receipt row UPDATE
triggers merely to acquire ownership. Contention is retryable. Nonterminal rows
with terminal payloads or completion stamps fail closed. SQLite regression
source models rewritten admission, renewal, success and failure; compilation
and runtime execution remain pending. This is not yet a DB-level immutable
receipt history or durable receipt retention policy.

## Durable consumer registry and ACK integrity (2026-11-28; source only)

Migration `m20261128_000210_fence_outbox_consumer_receipts` and standalone
schema setup share the same PostgreSQL and SQLite installer. Registry rows
are immutable; deletion requires a drained event type and matching receipts
for every extant source event. Consumer receipts require the exact source
event and active subscription on insertion, cannot be updated, and can be
deleted only after their parent is gone. PostgreSQL TRUNCATE is rejected.
SQLite exact-key duplicate INSERT/REPLACE preserves the original fact;
conflicting rowid replacement is rejected even with recursive triggers off.
Migration validates existing identity and source linkage and fails closed on
inconsistent history; previously retired subscriptions need not be recreated.

Canonical registration and fresh ACK require one affected row and exact
readback under the existing registry/source locks. An exact prior ACK is
recognized before INSERT; a suppressed new INSERT cannot masquerade as a
prior effect. Retirement also verifies the affected count. PostgreSQL direct
receipt insertion uses registry SHARE NOWAIT, source FOR UPDATE NOWAIT and
subscription FOR SHARE NOWAIT; registry retirement requires READ COMMITTED
and the event-table SHARE NOWAIT fence. SQLite serializes writers.
Stop/drain writers before migration and reconcile any rejected legacy data
offline; no automatic rewriting or deletion of ACK history.

A structurally correct ACK remains a trusted assertion by the owner. A role
with arbitrary INSERT access can fabricate a matching ACK without performing
the effect; generic Outbox cannot inspect another owner's private tables.
The caller must commit the owner effect and ACK together or roll back both.
External effects, credential isolation, event identity after retention and
consumer recovery remain separate boundaries. SQLite/PostgreSQL regression
sources are authored; compilation, migrations and contention tests are unrun.

## Durable owner-operation receipt authority (source only)

The forward migration `m20261128_000211_fence_outbox_owner_receipts` and standalone setup install immutable request identity, immutable terminal outcomes, a guarded processing/released/terminal state graph and a separate durable receipt-retention singleton. Fresh receipts start processing; creation time is no longer restamped after insertion (only the live lease is renewed). An insert delayed past its lease must roll back and retry. SQLite ignores scoped-key insert collisions rather than allowing REPLACE to erase history; UUID aliases and conflicting rowids are rejected. Admission checks the preserved request after a zero-row insert. Terminal receipt deletion requires the enabled durable window, matching SQL time floor and terminal completion age; nonterminal receipts cannot be deleted, and PostgreSQL TRUNCATE is rejected. The first retention invocation binds either a positive whole-millisecond window or indefinite retention; configuration mismatch fails closed. Pruning locks policy before receipt rows and rechecks SQL time after candidate locks; reverse-order PostgreSQL direct DELETE uses NOWAIT. Reconfigure the existing policy only with drained pruners and aligned settings. Old binaries must be stopped before migration; invalid legacy receipt shapes require offline reconciliation rather than automatic data loss. Structurally legal DML is still a trusted assertion, not proof of owner effects or an independent privilege boundary. SQLite regression sources updated; compilation and runtime verification remain pending.
