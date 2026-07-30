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

## FFA/FBA status block

- FFA status: `in_progress`
- FBA status: `boundary_ready`
- Structural shape: `core_transport_ui`
- FBA provider contract: `OutboxRelayPort` / `outbox.relay_control.v1` in
  `crates/rustok-outbox/contracts/outbox-fba-registry.json`.
- Static and runtime-order evidence:
  `crates/rustok-outbox/contracts/evidence/outbox-contract-test-static-matrix.json`
  and `crates/rustok-outbox/contracts/evidence/outbox-provider-runtime-order-smoke.json`.
- `npm run verify:outbox:admin-boundary` and `npm run verify:outbox:fba` lock
  the UI boundary, provider metadata, and owner-service invocation order.
- The server relay worker consumes `OutboxRelayPort::process_pending_once` with
  a service actor, deadline, and per-tick idempotency key; it does not invoke
  the relay service method directly.
- Transactional outbox integration tests live in this owner crate; the
  foundational `rustok-core` crate remains independent from outbox runtime
  implementations, including in its development dependency graph.

## Open results

1. **Define durable consumer completion.** Outbox transport acceptance is not the
   same as consumer projection completion. Module-local handlers currently run on an
   in-memory broadcast bus after relay acceptance; lag or final handler failure can
   leave Search and other projections stale while `sys_events` is terminal. Define a
   consumer-owned durable inbox/job/receipt boundary or a durable transport consumer
   contract without making Outbox own domain side effects.
   **Depends on:** event-flow ownership, consumer idempotency, remote/local topology,
   and acknowledgment semantics.
   **Done when:** relay terminal state, transport acknowledgment, and consumer terminal
   state are separately explicit; failures survive restart and are observable/replayable.

2. **Execute relay, backlog, retry, and DLQ runtime contracts.** Replace static
   evidence with targeted provider execution and fallback proof for relay
   control before any FBA promotion.
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
   local fan-out loss, consumer failure, and retry failures with an owner-specific
   recovery procedure.

## Verification

- `npm run verify:outbox:admin-boundary`
- `npm run test:verify:outbox:admin-boundary`
- `npm run verify:outbox:fba`
- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- Targeted transactional publish, relay, retry, DLQ, local fan-out, handler lag,
  restart, and durable consumer completion tests.

## Change rules

1. Keep transactional publishing and relay policy in this module.
2. Do not make Outbox authoritative for domain-consumer side effects; publish a
   neutral durable consumer-completion contract instead.
3. Update root/local docs and `rustok-module.toml` with a public event-runtime
   contract change.
4. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.

## Periodic release verification handoff

- Cycle: `cycle-001`
- Status: `in_progress`
- Last verified at (UTC): `2026-07-30`
- Scope inspected: `owner README, module docs, current implementation plan, Search carried delivery blocker and server relay/local fan-out composition`
- Findings: `P0=0, P1=1, P2=0, P3=0`
- Fixed in this pass: `none yet; verification just resumed`
- Remaining risks or blockers: `carried P1: relay/remote transport success can be terminal before an in-process module handler succeeds; final handler errors and broadcast lag are not durably retained or replayed`
- Evidence: `Search owner/provider and server EventRuntime/EventDispatcher source inspection; current Outbox owner docs and verification cursor read before deeper relay/storage audit`
- Next action: `inspect sys_events claim/dispatch state transitions, relay acknowledgment, retry/DLQ, server readiness and local fan-out semantics; define and fix the owner-correct durable completion boundary where feasible`
- Resume command: `cargo xtask module validate outbox && cargo xtask module test outbox && npm run verify:outbox:fba`
