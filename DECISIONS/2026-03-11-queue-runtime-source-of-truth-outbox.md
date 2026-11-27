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
