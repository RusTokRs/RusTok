# Queue runtime source of truth: `rustok-outbox` + `event_transport_factory`

- Date: 2026-03-11
- Status: Accepted

## Context

A generic queue/jobs subsystem can provide background execution, but it does
not fully cover the requirements of the current RusToK event-path for production:

1. **Transactional outbox as a write-side invariant**: the event must be committed in the same DB transaction as the domain operation (via `sys_events`), with atomic commit/rollback guarantees.
2. **Replay procedures as a first-class capability**: a standard operational path is needed for re-queuing events from DLQ/backlog without workaround migration scripts.
3. **DLQ and observability at the event lifecycle level**: explicit statuses, retry-attempts, and diagnostics are needed in the same entities where the outbox lives.
4. **Transport switching without changing the write-side**: the runtime must support independent relay target selection (`local|iggy`) with a fixed `transport = outbox`.

For these requirements, the codebase already has a specialized layer `rustok-outbox` and a transport factory in the server runtime. A parallel production job path would create contract divergence (two retry models, two paths for replay/DLQ, different observability points) and increase the risk of incidents during operation.

## Decision

1. **Source of truth for queues and event delivery-path in production** is fixed to:
   - `crates/modules/rustok-outbox` (outbox persistence, relay semantics, DLQ/retry lifecycle);
   - `apps/server/src/services/event_transport_factory.rs` (single runtime point for transport/relay target selection).
2. A generic queue/jobs system is allowed only as an auxiliary non-production mechanism (utility/background tasks) that does not duplicate the event delivery-path.
3. Any changes to queue/event runtime must be compatible with the outbox-first contract and pass through the specified source-of-truth components.

## Consequences

**Positives**
- Single production-path for outbox/retry/DLQ/replay without architectural fork.
- Predictable observability model and unified runbook procedures.
- Safe transport switching at the relay level without rewriting the write-side.

**Limitations and prohibitions**
- It is prohibited to introduce a **parallel production-path** through a generic jobs/queue system for domain event delivery without a separate ADR that explicitly describes the migration/cutover plan and rollback.

**Follow-up**
- Maintain references to this ADR in server runtime governance documents and in the central event architecture as a policy anchor.

## Relay execution boundary

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

## Distributed execution admission (2026-11-27)

The outbox_execution_policy singleton owns a per-database-schema budget
(max_in_flight, default 8, range 1..1024) and the shared claim_ttl_ms.
The first updated relay binds the TTL; subsequent embedded and standalone
relays must match it before starting their brokers or claiming work.
The first binding verifies both persisted TTL and cap before commit. Claim
and refresh verify the complete event transition under the admission fence;
refresh locks and compares the exact source snapshot without synthetic event
updates. A rewritten transition rolls back before dispatch. SQLite regression
sources cover policy and claim rewriting; compilation and runtime checks are pending.
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

A changed bound route is a terminal pre-publication validation failure:
move directly to DLQ and release the exact claim without creating a new
dispatch intent. Retrying cannot change the immutable route; a charged
pending retry without an open intent violates the event-state guard.
Preserve the original checkpoint and attempt history. Reconcile the
deployment route before explicit DLQ replay. Regression source is unrun.
Checkpoint-enabled primary publication refuses without its relay task scope;
the test-only uncheckpointed transport retains ordinary direct behavior.
The server regression exercises local failure, relay reconstruction and
confirmed-primary skipping with a transport double (not a live broker).

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

## Exact temporal observation and transport ACK admission (source only)

Independent clock observations reserve the singleton using PostgreSQL row
ownership or a SQLite zero-row writer reservation, without firing a synthetic
row UPDATE. The observed millisecond must be read back exactly before commit;
a suppressed or rewritten advance is an error, not durable clock evidence.
The unclaimed transport ACK now acquires the event row with PostgreSQL
FOR UPDATE NOWAIT (SQLite writer reservation) before sampling SQL time and
compares the full expected event model before commit. Contention is retryable
at the caller; an existing claim or open intent still blocks the ACK. This
prevents an ACK waiting on source ownership from backdating retention age.
Neither path acquires a global clock writer lock while holding event or owner
rows. SQLite fault-injection and PostgreSQL source-contention regressions are
authored, not executed. Independent clock jumps and privileged DDL/restore
remain outside this guarantee.
