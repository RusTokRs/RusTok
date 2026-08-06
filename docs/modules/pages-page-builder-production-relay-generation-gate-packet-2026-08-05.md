# Pages / Page Builder Production Relay Generation Gate Packet

Date: 2026-08-05
Status: source-ready / execution-pending / FFA-FBA-not-promoted
Scope: production event delivery → synchronous Pages generation rotation → downstream transport → asynchronous duplicate-safe listener

## Corrected cursor

The topology correction after PR #2995 established that the retained continuity harness used a custom synchronous relay target, while the production server accepted an event into its relay target before the asynchronous module dispatcher completed `PageCacheInvalidationEventHandler`.

This slice closes that source gap in production code. It does not replace the module listener or introduce a durable listener-receipt protocol.

## Production transport placement

`TenantGenerationDeliveryGate` is already mounted by `tenant_generation_transport` for the OutboxLocal and OutboxIggy delivery profiles.

With `mod-pages`, the gate now owns this ordering:

```text
canonical tenant listener readiness
  → PageCacheInvalidationEventHandler::handles
  → PageCacheInvalidationEventHandler::handle
  → downstream EventTransport::publish
  → OutboxRelay may mark the row dispatched
```

Non-Pages events continue directly to the existing downstream transport after the established listener-readiness gate.

## Shared idempotency

Every `ServerPagesCachePort` uses one process-bounded `BoundedCacheEventDedupe` keyed by the stable event UUID.

For a first event attempt:

1. acquire the bounded per-event serialization stripe;
2. confirm the UUID is not already successful;
3. bump every owner-declared namespace generation;
4. validate the event/correlation-bound receipt;
5. record the event UUID as successful;
6. continue to downstream delivery.

The UUID is not recorded when generation rotation or receipt validation fails, so the transport returns an error and the publisher or relay can retry.

The UUID is recorded before downstream delivery. If the downstream transport rejects after generation rotation, a retry repeats delivery but reads a valid current receipt without rotating the same event again.

## Asynchronous listener compatibility

The Pages module listener remains registered for both profiles. This preserves the established module-owned listener model.

When the listener later receives an event already handled by the synchronous gate, it constructs another `ServerPagesCachePort`, resolves the same process-bounded successful-event set, and returns a valid current receipt without another generation bump.

A process restart intentionally loses this bounded optimization. A replay after restart may rotate an additional generation, which is conservative and does not expose stale data.

## Retained source tests

`apps/server/src/services/pages_cache_invalidation.rs` retains:

- full-scope `Published` rotation;
- mutable-only `Updated` rotation;
- duplicate delivery through a separately constructed provider with no second bump;
- initial generation and cache byte round-trip behavior.

`apps/server/src/services/tenant_generation_delivery_gate.rs` retains:

- canonical local listener readiness before downstream delivery;
- downstream rejection after Pages rotation;
- successful delivery retry without a second rotation;
- later asynchronous Pages handler delivery without another rotation.

## Boundaries

This slice does not:

- change Page Builder review, sanitizer, materialization or artifact contracts;
- change Pages lifecycle event emission or scope policy;
- change cache namespace names, key shapes, TTLs or maximum capacity;
- change outbox, Pages or Page Builder database schemas;
- remove the asynchronous Pages listener;
- add durable listener receipts;
- claim process-restart exact-once invalidation;
- claim tests, Cargo, formatting, verifiers, workflows, CI or tenant rollout were executed;
- promote FFA or FBA status.

## Evidence

Machine-readable source evidence:

- `crates/rustok-pages/contracts/evidence/pages-production-relay-generation-gate-source.json`;
- `crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs`.

The execution list remains empty and every observed validation field remains false.

## Suggested maintainer validation

Intentionally not run in this slice:

```bash
node crates/rustok-pages/scripts/verify/verify-pages-production-relay-generation-gate.mjs
cargo test -p rustok-server --features mod-pages services::pages_cache_invalidation -- --nocapture
cargo test -p rustok-server --features mod-pages services::tenant_generation_delivery_gate -- --nocapture
cargo check -p rustok-server --features mod-pages --all-targets
cargo check -p rustok-pages --all-targets
cargo check -p rustok-outbox --all-targets
```

After those checks, rerun the registered native storefront relay continuity and PostgreSQL relay-restart packets against this production gate before promoting the shared parity cursor.
