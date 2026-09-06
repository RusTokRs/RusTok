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
`logs:manage` and looks up the event by both id and tenant. Tenant admins cannot
select another tenant or inspect platform-global relay state through these
surfaces.

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
  `crates/rustok-outbox/contracts/evidence/blog-comments-audit-relay-postgres-source.json`.
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
