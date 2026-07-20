# Implementation plan for `rustok-outbox`

## Current state

`rustok-outbox` owns transactional event publishing, relay, retry, DLQ
semantics, and relay-worker control. The server and other modules consume this
runtime through owner contracts; they must not reimplement event delivery or
relay lifecycle.

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

1. **Execute relay, backlog, retry, and DLQ runtime contracts.** Replace static
   evidence with targeted provider execution and fallback proof for relay
   control before any FBA promotion.
   **Depends on:** a runtime-composed relay and representative delivery
   failures.
   **Done when:** transactional publish, retry, DLQ transition, degraded mode,
   and typed port errors are covered by executable tests.

2. **Prepare safe incremental operational adoption.** Define rollout,
   migration, tenant, RBAC, and security requirements that belong to relay
   control rather than to the host UI.
   **Depends on:** deployment topology and operator authorization model.
   **Done when:** staged enablement has explicit guardrails, permissions, and a
   rollback path without duplicated relay ownership.

3. **Maintain observability and incident guidance with delivery semantics.**
   Update metrics, alerting, and the runbook whenever relay/backlog/DLQ behavior
   changes.
   **Depends on:** the changed outbox runtime contract.
   **Done when:** operators can identify stalled relay, growing backlog, DLQ,
   and retry failures with an owner-specific recovery procedure.

## Verification

- `npm run verify:outbox:admin-boundary`
- `npm run test:verify:outbox:admin-boundary`
- `npm run verify:outbox:fba`
- `cargo xtask module validate outbox`
- `cargo xtask module test outbox`
- Targeted transactional publish, relay, retry, and DLQ runtime tests.

## Change rules

1. Keep transactional publishing and relay policy in this module.
2. Update root/local docs and `rustok-module.toml` with a public event-runtime
   contract change.
3. Update this status block and `docs/modules/registry.md` with an FFA/FBA
   boundary change.
