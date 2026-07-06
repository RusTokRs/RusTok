# Implementation plan for `rustok-iggy`

Status: transport baseline already exists; the main work going forward is not
about creating abstractions from scratch, but about bringing the real Iggy integration path to
production-grade level.

## Execution checkpoint

- Current phase: real_integration_hardening
- Last checkpoint: no-compile increment: `ack_consumed` now validates stream/topic/partition metadata before ack, to avoid committing an opaque token from a different cursor; source-level unit assertions added for the mismatch guardrail.
- Next step: replace simulated connector ack with real SDK subscriber ack/offset commit path and add actual targeted test evidence.
- Open blockers: compile/test evidence deferred due to explicit iteration constraint: no compilations.
- Hand-off notes for next agent: The next increment should connect real SDK metadata extraction with `validate_connector_metadata`/`ack_consumed`, then run targeted tests when the compilation restriction is lifted.
- Last updated at (UTC): 2026-06-20T00:00:00Z

## Scope of work

- keep `rustok-iggy` as a transport crate over `rustok-iggy-connector`;
- synchronize serialization/topology/DLQ/replay contracts and local docs;
- prevent mixing transport logic with connector lifecycle.

## Current state

- `IggyTransport` already implements `EventTransport`;
- JSON/Postcard serialization, topology helpers, consumer groups, DLQ and replay abstractions are already separated;
- connection mode switching and low-level I/O are already moved to `rustok-iggy-connector`;
- part of the production-grade integration semantics still requires deepening the real SDK path.

## Stages

### 1. Contract stability

- [x] lock transport boundary over the connector crate;
- [x] keep transport-facing abstractions inside `rustok-iggy`;
- [x] maintain sync between transport contracts, connector expectations and local docs.

### 2. Real integration hardening

- [ ] bring full Iggy SDK integration path;
- [ ] close real consumption, offset management, DLQ movement and replay flows;
  - [x] add first transport-owned consume path over connector `subscribe` and serializer deserialize;
  - [x] add offset/ack metadata and wire-up for DLQ/replay movement;
    - [x] consume path carries connector offset/opaque ack metadata into `ConsumedEvent`;
    - [x] transport exposes `ack_consumed`; DLQ entries retain connector metadata and retry republishes with retry-limit validation;
    - [x] replay config validates offset windows and records planned offsets for bounded replay runs;
    - [x] `ack_consumed` rejects connector metadata from another stream/topic/partition before invoking connector ack;
- [ ] cover performance/recovery/security edge-cases with targeted tests and drills.

### 3. Operability

- [ ] evolve metrics, health checks and runbooks for production transport usage;
- [ ] keep local docs synchronized with connector docs and event-system guidance;
- [ ] document transport guarantees simultaneously with changing runtime surface.

## Verification

contract tests cover all public use-cases

- [ ] contract tests cover all public use-case orchestration and surface contracts.
- targeted compile/tests for configuration, serialization, topology, consumer groups and replay/DLQ contracts (current no-compile increment added fake-connector unit coverage, execution deferred);
- integration tests for real Iggy backend path;
- docs sync between transport and connector layers.

## Update rules

1. When changing transport contract, update this file first.
2. When changing public surface, synchronize `README.md` and `docs/README.md`.
3. When changing connector boundary, update related docs in `rustok-iggy-connector`.


## Quality backlog

- [x] Update test coverage for key module scenarios: added roundtrip deserialize and consume_next fake-connector tests.
- [x] Add DLQ/replay tests over offset/ack metadata for transport-owned metadata plumbing (real SDK ack evidence remains open).
- [x] Add source-level ack metadata mismatch guardrail for `ConsumedEvent`/`ack_consumed` (test execution deferred without compilations).
- [ ] Verify completeness and currency of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
