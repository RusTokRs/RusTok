# Queue runtime source of truth: `rustok-outbox` + `event_transport_factory`

- Date: 2026-03-11
- Status: Accepted

## Context

A generic queue/jobs subsystem can provide background execution, but it does
not fully cover the requirements of the current RusToK event-path for production:

1. **Transactional outbox as a write-side invariant**: the event must be committed in the same DB transaction as the domain operation (via `sys_events`), with atomic commit/rollback guarantees.
2. **Replay procedures as a first-class capability**: a standard operational path is needed for re-queuing events from DLQ/backlog without workaround migration scripts.
3. **DLQ and observability at the event lifecycle level**: explicit statuses, retry-attempts, and diagnostics are needed in the same entities where the outbox lives.
4. **Transport switching without changing the write-side**: the runtime must support independent relay target selection (`memory|iggy|...`) with a fixed `transport = outbox`.

For these requirements, the codebase already has a specialized layer `rustok-outbox` and a transport factory in the server runtime. A parallel production job path would create contract divergence (two retry models, two paths for replay/DLQ, different observability points) and increase the risk of incidents during operation.

## Decision

1. **Source of truth for queues and event delivery-path in production** is fixed to:
   - `crates/rustok-outbox` (outbox persistence, relay semantics, DLQ/retry lifecycle);
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
