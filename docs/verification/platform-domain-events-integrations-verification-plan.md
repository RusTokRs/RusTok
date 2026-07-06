---
id: doc://docs/verification/platform-domain-events-integrations-verification-plan.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Platform Verification Plan: Events, Domains and Integrations

- **Status:** current detailed checklist
- **Scope:** domain events, outbox/runtime transport, inter-module connections, integration boundaries
- **Note:** API and UI surfaces are checked in separate verification plans; event/runtime contract remains here

---

## Current Scoped Contract

The event/runtime layer must remain consistent with the current-state model:

- canonical event contracts live in `rustok-events`
- transactional delivery contract lives in `rustok-outbox`
- publishers own the semantic meaning of their events
- consumers remain idempotent and replay-safe

## Phase 1. Event Runtime

### 1.1 Runtime bootstrap

**Files:**
- `apps/server/src/services/event_transport_factory.rs`
- `apps/server/src/services/event_bus.rs`
- `crates/rustok-outbox/`
- `crates/rustok-iggy/`

- [ ] Server bootstrap raises the current event runtime.
- [ ] Transport mode is consistent with current settings and runtime wiring.
- [ ] `rustok-outbox` remains the production-first delivery path where transactional consistency is required.
- [ ] Additional transport layers do not replace the canonical outbox contract.

### 1.2 Transactional publish path

- [ ] Domain write path and outbox write happen in the same transaction where the contract requires it.
- [ ] Inter-module events are not published outside the canonical transactional path without an explicit reason.
- [ ] Runtime docs and local docs of the publisher match the actual publish path.

## Phase 2. Domain Event Ownership

### 2.1 Publishers

- [ ] Event family ownership matches owning module/service layer.
- [ ] Host layer does not become a hidden publisher of a module-owned event family.
- [ ] Shared helper events do not grow into a universal substitute for typed domain events.

### 2.2 Consumers

- [ ] Consumers update projections and downstream state idempotently.
- [ ] Replay and recovery paths remain valid.
- [ ] Consumer path does not break module boundaries.

## Phase 3. Inter-Module Connections

### 3.1 Dependency discipline

- [ ] Inter-module dependencies match `modules.toml`, local docs and runtime wiring.
- [ ] A new integration path does not create hidden direct connectivity between modules where an event-driven contract is needed.
- [ ] Capability/support crates are not passed off as platform modules in the integration graph.

## Phase 4. Read-Side and Integrations

### 4.1 Index/read consumers

- [ ] Read-side consumers are consistent with `rustok-index` and the event-flow contract.
- [ ] External integration path does not replace the canonical internal event flow.
- [ ] Routing/cache/index updates tied to events are described in owning component docs.

## Phase 5. Targeted Local Checks

### 5.1 Minimum

- [ ] targeted `cargo check` / `cargo test` for affected publishers/consumers
- [ ] targeted `xtask module test <slug>`, if module-owned event contract changes
- [ ] targeted runtime smoke, if transport wiring changes

## Open Blockers

- [ ] Record runtime-only blockers briefly, separate from the checklist itself.
- [ ] Do not turn this document into a list of historical incidents.

## Related Documents

- [Domain Event Flow Contract](../architecture/event-flow-contract.md)
- [Channels and Real-Time Surfaces](../architecture/channels.md)
- [Module Architecture](../architecture/modules.md)
- [Modular Platform Crate Registry](../modules/crates-registry.md)
