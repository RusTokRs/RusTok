---
id: doc://docs/architecture/event-flow-contract.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Domain Event Flow Contract

This document captures the canonical path of a `DomainEvent` in RusToK: from a domain
operation to updating downstream read-side state.

## Canonical Path

1. Domain/service layer performs a business operation.
2. The write-side state change and outbox write occur in a single transaction.
3. `rustok-outbox` delivers the event to the transport/runtime layer.
4. Registered consumers update projections, indexes and other
   downstream surfaces idempotently.
5. UI and API read the already consistent read-side state.

## Sources of Truth

- canonical event contracts live in `rustok-events`
- compatibility re-export may exist in `rustok-core`, but must not
  replace ownership
- transactional delivery contract lives in `rustok-outbox`
- consumer-specific semantics must be reflected in local docs of the publisher and
  consumer

## Component Roles

### Publisher

Publisher:

- owns the semantic meaning of the event
- defines mandatory payload fields
- publishes the event through the canonical write path

Publisher must not treat the event bus as its read-model API.

### Outbox/Runtime Layer

`rustok-outbox` is responsible for:

- transactional persistence
- retry/backoff
- delivery bookkeeping
- predictable runtime contract for consumers

`rustok-outbox` remains a `Core` module, not a support utility.

### Consumer

Consumer:

- must be idempotent
- must recompute its state from the source of truth, not from local
  assumptions
- must not break the publisher's write-side contract

## Module Event Listeners

Module-owned event listeners are published through the module's own runtime contract:

- `RusToKModule::register_event_listeners(...)` registers handlers in `ModuleEventListenerRegistry`;
- `apps/server` collects them through `ModuleRegistry::build_event_listeners(...)` and connects them to the common `EventDispatcher`;
- runtime dependencies for listeners are passed through `ModuleEventListenerContext` and `ModuleRuntimeExtensions`, not through host-owned manual wiring in `apps/server`.

This means the module owns its event consumers just as it owns its
`GraphQL`, `HTTP` and UI surfaces.

### What Is Not Considered an Event Listener

This contract does not include:

- cron/background jobs;
- relay workers;
- transport forwarders;
- long-running host maintenance tasks.

For example, `WorkflowCronScheduler` remains a separate background runtime path and is not
published as an `event_listener`.

## Content and Orchestration Events

A distinction must be made between:

- storage-owner domain events of a specific module
- orchestration/canonical-routing events
- helper/reindex events for legacy or shared paths

New scenarios should rely on typed storage-owner or orchestration events,
rather than endlessly extending the shared helper surface.

## Commerce Events

For the commerce family, the same principle applies:

- ownership of the event belongs to the specific domain/service layer
- projections and index updates go through the consumer path
- transport/runtime does not replace domain ownership

## Platform Build Events

`rustok-build` owns platform build lifecycle events. A rollback is the explicit
`BuildRolledBack` transition, not another successful completion. Its canonical
root event is `build.rolled_back`: the payload binds the requested and restored
builds plus source and target releases, while `EventEnvelope.actor_id` carries
the verified actor. Server WebSocket and GraphQL subscriptions adapt that same
owner event and must preserve those facts.

## Retry and Resilience

For event flow, the following are mandatory:

- finite and observable retry
- backoff
- idempotent consumer operations
- replay-safe behavior

If a consumer is not idempotent, it does not conform to the platform event contract.

Idempotence must survive process restart and therefore cannot be based
solely on in-memory state. For business-effect consumers, a durable
processing key and DB constraint are needed. For example, event-triggered workflow
execution uses `(workflow_id, trigger_event_id)` and on redelivery
returns the already existing execution without re-running steps.

## What Not To Do

- do not publish cross-module events bypassing the outbox if transactional
  consistency is needed
- do not treat event payload as the only long-lived storage format
- do not transfer canonical ownership of events to the host layer
- do not build a new consumer path without updating local docs and the central contract

## When to Update This Document

This central contract needs to be updated if any of the following changes:

- ownership of an event family
- the canonical publisher path
- a consumer class
- retry/runtime semantics
- the role of `rustok-events` or `rustok-outbox`

When doing so, first update the local docs of the publisher and consumer, then the central
docs.

## Related Documents

- [Module Architecture](./modules.md)
- [Channels and Real-time Surfaces](./channels.md)
- [Platform Diagrams](./diagram.md)
- [Module Platform Crate Registry](../modules/crates-registry.md)
