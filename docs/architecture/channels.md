---
id: doc://docs/architecture/channels.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Channels and Real-time Surfaces

This document captures the role of real-time channels in RusToK and their place in the overall
transport architecture.

## Purpose

Real-time channels are used where a push delivery format of events over
a long-lived connection is needed:

- streaming status of build/runtime operations
- live progress for long-running tasks
- future notification/event streams, if they require push delivery

Channels do not replace GraphQL, REST, or the event bus. They are a separate transport surface
for live delivery.

## Current Baseline

At the current layer, real-time channels are built on top of WebSocket routing in
`apps/server`.

Canonical rules:

- the websocket route lives in the host layer
- the payload contract must be typed and documented
- auth/tenant/RBAC policy is applied before granting a channel or in the handshake path
- the channel must not become a source of truth for domain state

## Where the Boundary Lies

### Host Layer

`apps/server` is responsible for:

- websocket handshake
- connection lifecycle
- auth/session validation
- tenant context
- fan-out transport and shutdown behavior

### Module / Service Layer

The module or runtime service is responsible for:

- generating typed events
- publishing to the hub/broadcast layer
- semantic contract of the payload

### Central Event Flow

WebSocket channel must not replace the event runtime:

- domain events go through `rustok-outbox` and `rustok-events`
- read-side and projections are updated through event flow
- websocket is only needed for live delivery of current status or progress

## Build/Event-streaming

Build/runtime progress channel remains a valid scenario if:

- there is a typed event contract
- payload serializes stably
- reconnect and lag do not break the semantic contract
- the client can restore state through canonical API if events were missed

This is important: WebSocket stream must not be the only source of state.

## Wire Contract

For WebSocket payload, the following minimum applies:

- explicit `type`
- stable machine-readable payload
- minimal set of mandatory fields for the consumer
- compatibility with tracing/observability

If the channel becomes a long-term platform contract, its payload must be
described in the local docs of the owning component.

## Shutdown and Fault Tolerance

The channel must correctly handle:

- client close
- graceful host shutdown
- lag/backpressure
- temporary publisher unavailability

Transport channel failure must not break a write-side operation if it has already
completed and been confirmed by the canonical API/state.

## What Not To Do

- do not use websocket as the only source of domain state
- do not bypass auth/tenant/RBAC policy for convenience
- do not publish ad-hoc JSON without a typed contract
- do not transfer ownership of domain events from event runtime to websocket hub

## Related Documents

- [Domain Event Flow Contract](./event-flow-contract.md)
- [API Architecture](./api.md)
- [Routing and Transport Layer Boundaries](./routing.md)
- [Platform Architecture Overview](./overview.md)
